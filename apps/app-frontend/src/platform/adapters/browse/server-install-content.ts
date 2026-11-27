import type { Server } from '@amberite/contracts/core'
import type { Labrinth } from '@modrinth/api-client'
import {
	addServerVersions,
	type BrowseInstallPlan,
	type BrowseInstallProject,
	type BrowseSelectedProject,
	createContext,
	type CreationFlowContextValue,
	flushStoredServerAddonInstallQueue,
	getServerAddonInstallPlanProjectIds,
	getStoredServerAddonInstallQueue,
	getTargetInstallPreferences,
	injectModrinthClient,
	injectNotificationManager,
	injectServerOnboardingFlow,
	instanceContentQueries,
	readStoredServerInstallQueue,
	resolveServerAddonInstallPlans,
	resolveServerPlatform,
	type ServerPlatform,
	serverContentQueryKeys,
	serverDetailQueryOptions,
	serverKeys,
	setUpServer,
	type UpdateChannelPolicy,
	writeStoredServerInstallQueue,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, type ComputedRef, nextTick, type Ref, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { config as appConfig } from '@/config'
import { useBreadcrumbServerId } from '@/shared/breadcrumbs'

type ServerFlowFrom = 'onboarding' | 'reset-server'

// Queued from search results and from project pages, so only the fields install needs.
type InstallableSearchResult = BrowseInstallProject & {
	name: string
	installing?: boolean
	installed?: boolean
}

export interface ServerModpackSelectionRequest {
	contentType?: string
	projectId: string
	versionId: string
	name: string
	iconUrl?: string
}

interface ServerSetupModalHandle {
	show: () => void | Promise<void>
	hide: () => void
	ctx?: CreationFlowContextValue | null
}

export interface ServerInstallContentContext {
	serverIdQuery: ComputedRef<string | null>
	browseFrom: ComputedRef<string | null>
	serverFlowFrom: ComputedRef<ServerFlowFrom | null>
	isFromWorlds: ComputedRef<boolean>
	isServerContext: ComputedRef<boolean>
	isSetupServerContext: ComputedRef<boolean>
	serverContextServerData: Ref<Server | null>
	serverContextPlatform: ComputedRef<ServerPlatform>
	serverContentProjectIds: Ref<Set<string>>
	queuedServerInstallRootProjectIds: ComputedRef<Set<string>>
	queuedServerInstallProjectIds: ComputedRef<Set<string>>
	queuedServerInstallCount: ComputedRef<number>
	selectedServerInstallProjects: ComputedRef<BrowseSelectedProject[]>
	isInstallingQueuedServerInstalls: Ref<boolean>
	queuedInstallProgress: Ref<{ completed: number; total: number }>
	serverBackUrl: ComputedRef<string>
	serverBackLabel: ComputedRef<string>
	serverBrowseHeading: ComputedRef<string>
	clearQueuedServerInstalls: () => void
	removeQueuedServerInstall: (projectId: string) => void
	flushQueuedServerInstalls: () => Promise<boolean>
	discardQueuedServerInstallsAndBack: () => Promise<void>
	installQueuedServerInstallsAndBack: () => Promise<boolean>
	initServerContext: () => Promise<void>
	watchServerContextChanges: () => void
	getServerProjectVersions: (projectId: string) => Promise<{ id: string }[]>
	getServerUpdateChannel: () => Promise<UpdateChannelPolicy | undefined>
	enforceServerSetupRoute: (currentProjectType: string | undefined) => void
	getQueuedServerInstallPlans: () => Map<string, BrowseInstallPlan<InstallableSearchResult>>
	setQueuedServerInstallPlans: (
		plans: Map<string, BrowseInstallPlan<InstallableSearchResult>>,
	) => void
	resolveQueuedServerInstallPlan: (
		plan: BrowseInstallPlan<InstallableSearchResult>,
	) => Promise<void>
	activeServerModpackInstallProjectId: Ref<string | null>
	openServerModpackInstallFlow: (request: ServerModpackSelectionRequest) => Promise<void>
	onServerFlowHide: () => void
	onServerFlowBack: () => void
	handleServerModpackFlowCreate: (config: CreationFlowContextValue) => Promise<void>
	markServerProjectInstalled: (id: string) => void
}

export const [, provideServerInstallContent] = createContext<ServerInstallContentContext>(
	'Browse',
	'serverInstallContent',
)

function readQueryString(value: unknown): string | null {
	if (Array.isArray(value)) return value[0] ?? null
	return typeof value === 'string' && value.length > 0 ? value : null
}

export function useServerInstallContent(opts: {
	serverSetupModalRef: Ref<ServerSetupModalHandle | null>
	/**
	 * Falls back to the server in the breadcrumb trail when the route has no `sid`, so pages reached
	 * from a server panel install to that server.
	 */
	inheritBreadcrumbServer?: boolean
}) {
	const { serverSetupModalRef } = opts
	const breadcrumbServerId = opts.inheritBreadcrumbServer ? useBreadcrumbServerId() : null
	const route = useRoute()
	const router = useRouter()
	const client = injectModrinthClient()
	const onboardingFlow = injectServerOnboardingFlow()
	const { handleError } = injectNotificationManager()
	const queryClient = useQueryClient()

	const serverIdQuery = computed(
		() => readQueryString(route.query.sid) ?? breadcrumbServerId?.value ?? null,
	)
	const browseFrom = computed(() => readQueryString(route.query.from))
	const serverFlowFrom = computed<ServerFlowFrom | null>(() =>
		browseFrom.value === 'onboarding' || browseFrom.value === 'reset-server'
			? browseFrom.value
			: null,
	)

	const isFromWorlds = computed(() => browseFrom.value === 'worlds')
	const isServerContext = computed(() => !!serverIdQuery.value)
	const isSetupServerContext = computed(() => !!serverIdQuery.value && !!serverFlowFrom.value)
	function getCachedServer(serverId: string): Server | null {
		return (
			queryClient.getQueryData<Server>(serverKeys.detail(serverId)) ??
			queryClient
				.getQueryData<{ servers: Server[] }>(serverKeys.list())
				?.servers.find((server) => server.server_id === serverId) ??
			null
		)
	}

	async function ensureServer(serverId: string): Promise<Server> {
		return queryClient.ensureQueryData(serverDetailQueryOptions(client, serverId))
	}

	const initialServerId = serverIdQuery.value
	const serverContextServerData = ref<Server | null>(
		initialServerId ? getCachedServer(initialServerId) : null,
	)
	const serverContextPlatform = computed(() =>
		resolveServerPlatform(serverContextServerData.value, null),
	)
	const serverContentProjectIds = ref<Set<string>>(new Set())
	const queuedServerInstalls = ref<Map<string, BrowseInstallPlan<InstallableSearchResult>>>(
		new Map(),
	)
	const queuedServerInstallRootProjectIds = computed(
		() => new Set(queuedServerInstalls.value.keys()),
	)
	const queuedServerInstallProjectIds = computed(() =>
		getServerAddonInstallPlanProjectIds(queuedServerInstalls.value.values()),
	)
	const queuedServerInstallCount = computed(() => queuedServerInstalls.value.size)
	const selectedServerInstallProjects = computed<BrowseSelectedProject[]>(() =>
		Array.from(queuedServerInstalls.value.values()).map((plan) => ({
			id: plan.projectId,
			name: plan.project.name ?? 'Project',
			iconUrl: plan.project.icon_url ?? null,
		})),
	)
	const isInstallingQueuedServerInstalls = ref(false)
	const activeServerModpackInstallProjectId = ref<string | null>(null)
	const queuedInstallProgress = ref({ completed: 0, total: 0 })
	const serverBackUrl = computed(() => {
		const sid = serverIdQuery.value
		if (!sid) return '/hosting/manage'
		if (serverFlowFrom.value === 'onboarding') {
			return `/hosting/manage/${sid}?resumeModal=setup-type`
		}
		if (serverFlowFrom.value === 'reset-server') {
			return `/hosting/manage/${sid}?openSettings=installation`
		}
		return `/hosting/manage/${sid}/content`
	})
	const serverBackLabel = computed(() => {
		if (serverFlowFrom.value === 'onboarding') return 'Back to setup'
		if (serverFlowFrom.value === 'reset-server') return 'Cancel reset'
		return 'Back to server'
	})
	const serverBrowseHeading = computed(() => {
		if (serverFlowFrom.value === 'reset-server') {
			return 'Selecting content to install after reset'
		}
		return 'Installing content'
	})

	async function refreshServerInstalledContent(serverId: string) {
		try {
			const { items } = await queryClient.fetchQuery(
				instanceContentQueries({
					client: await client.cores.serverRoutes(serverId),
					instanceId: serverId,
					queryKeys: serverContentQueryKeys,
				}).content(),
			)
			if (serverIdQuery.value !== serverId) return
			serverContentProjectIds.value = new Set(
				items.flatMap((item) => (item.project ? [item.project.id] : [])),
			)
		} catch (err) {
			handleError(err as Error)
		}
	}

	async function initServerContext() {
		const sid = serverIdQuery.value
		if (!sid) return

		try {
			const server = await ensureServer(sid)
			if (serverIdQuery.value === sid) {
				serverContextServerData.value = server
			}
		} catch (err) {
			handleError(err as Error)
		}

		if (serverIdQuery.value !== sid) return

		queuedServerInstalls.value = readStoredServerInstallQueue(sid, sid)
		await refreshServerInstalledContent(sid)
	}

	function watchServerContextChanges() {
		watch(serverIdQuery, async (sid, prevSid) => {
			if (!sid) {
				serverContextServerData.value = null
				serverContentProjectIds.value = new Set()
				setQueuedServerInstallPlans(new Map())
				return
			}

			if (sid === prevSid) return
			serverContentProjectIds.value = new Set()
			queuedServerInstalls.value = readStoredServerInstallQueue(sid, sid)
			serverContextServerData.value = getCachedServer(sid)
			try {
				const server = await ensureServer(sid)
				if (serverIdQuery.value === sid) {
					serverContextServerData.value = server
				}
			} catch (err) {
				handleError(err as Error)
			}
			await refreshServerInstalledContent(sid)
		})
	}

	function enforceServerSetupRoute(currentProjectType: string | undefined) {
		if (
			!isSetupServerContext.value ||
			['modpack', 'mod', 'plugin', 'datapack'].includes(currentProjectType ?? '')
		)
			return
		router.replace({
			path: '/browse/modpack',
			query: route.query,
		})
	}

	async function getServerProjectVersions(projectId: string) {
		const versions = await client.labrinth.versions_v3.getProjectVersions(projectId)
		return versions.map((version) => ({ id: version.id }))
	}

	/**
	 * Release channel configured on the target world, used to avoid installing alpha or beta
	 * versions when the world is set to release.
	 */
	async function getServerUpdateChannel(): Promise<UpdateChannelPolicy | undefined> {
		const sid = serverIdQuery.value
		if (!sid) return undefined
		return (await ensureServer(sid)).update_channel
	}

	async function openServerModpackInstallFlow(request: ServerModpackSelectionRequest) {
		if (!serverIdQuery.value) throw new Error('Missing server context')

		activeServerModpackInstallProjectId.value = request.projectId
		try {
			if (serverFlowFrom.value === 'onboarding') {
				await onboardingFlow.open({
					serverId: serverIdQuery.value,
					siteUrl: appConfig.accountSiteUrl,
					project: request,
					backToBrowse: true,
					onHide: onServerFlowHide,
				})
				return
			}

			const modalInstance = serverSetupModalRef.value
			if (!modalInstance) throw new Error('Server setup modal is unavailable')

			await modalInstance.show()
			await nextTick()

			const ctx = modalInstance.ctx
			if (!ctx) throw new Error('Server setup modal is unavailable')

			if (request.contentType && request.contentType !== 'modpack') {
				await ctx.selectProject(request.projectId, request.contentType, request.versionId)
				return
			}

			ctx.setupType.value = 'modpack'
			ctx.modpackSelection.value = {
				projectId: request.projectId,
				versionId: request.versionId,
				name: request.name,
				iconUrl: request.iconUrl,
			}
			ctx.modal.value?.setStage('final-config')
		} catch (error) {
			onServerFlowHide()
			throw error
		}
	}

	function onServerFlowHide() {
		activeServerModpackInstallProjectId.value = null
	}

	function clearQueuedServerInstalls() {
		setQueuedServerInstallPlans(new Map())
	}

	function removeQueuedServerInstall(projectId: string) {
		const nextPlans = new Map(queuedServerInstalls.value)
		nextPlans.delete(projectId)
		setQueuedServerInstallPlans(nextPlans)
	}

	function setStoredServerInstallPlans(
		serverId: string,
		plans: Map<string, BrowseInstallPlan<InstallableSearchResult>>,
	) {
		if (serverId === serverIdQuery.value) queuedServerInstalls.value = plans
		writeStoredServerInstallQueue(serverId, serverId, plans)
	}

	function toResolvePreferences(
		preferences?: BrowseInstallPlan<InstallableSearchResult>['preferences'],
	): Labrinth.Content.v3.ResolutionPreferences {
		return {
			game_versions: preferences?.gameVersions,
			loaders: preferences?.loaders,
		}
	}

	async function resolveAddonPlan(
		plan: BrowseInstallPlan<InstallableSearchResult>,
		existingProjectIds: string[],
	) {
		const target = getTargetInstallPreferences(
			{
				gameVersion: serverContextPlatform.value.gameVersion,
				loader: serverContextPlatform.value.loader,
			},
			plan.contentType,
		)
		const resolved = await client.labrinth.content_v3.resolve({
			project_id: plan.projectId,
			version_id: plan.versionId,
			content_type: plan.contentType as Labrinth.Content.v3.ContentType,
			selected: toResolvePreferences(plan.preferences),
			target: toResolvePreferences(target),
			existing_project_ids: existingProjectIds,
			update_channel: await getServerUpdateChannel(),
		})

		return [resolved.primary, ...resolved.dependencies].map((item) => ({
			projectId: item.project_id,
			versionId: item.version_id,
		}))
	}

	async function resolveQueuedServerInstallPlan(plan: BrowseInstallPlan<InstallableSearchResult>) {
		const resolvedContent = await resolveAddonPlan(plan, Array.from(serverContentProjectIds.value))
		const storedPlan = queuedServerInstalls.value.get(plan.projectId)
		if (!storedPlan || storedPlan.versionId !== plan.versionId) return

		const nextPlans = new Map(queuedServerInstalls.value)
		nextPlans.set(plan.projectId, { ...storedPlan, resolvedContent })
		setQueuedServerInstallPlans(nextPlans)
	}

	async function resolveQueuedAddonPlans(plans: BrowseInstallPlan<InstallableSearchResult>[]) {
		return await resolveServerAddonInstallPlans({
			plans,
			existingProjectIds: serverContentProjectIds.value,
			resolvePlan: resolveAddonPlan,
		})
	}

	async function flushQueuedServerInstalls(serverId: string | null = serverIdQuery.value) {
		if (isInstallingQueuedServerInstalls.value) return false

		if (!serverId) {
			handleError(new Error('No server is available for install.'))
			return false
		}

		// Upstream keys the queue by server and world; a Core server's one world is the server.
		const queuedPlans = getStoredServerAddonInstallQueue<InstallableSearchResult>(
			serverId,
			serverId,
		)
		if (queuedPlans.size === 0) return true

		isInstallingQueuedServerInstalls.value = true
		queuedInstallProgress.value = {
			completed: 0,
			total: queuedPlans.size,
		}

		try {
			const result = await flushStoredServerAddonInstallQueue<InstallableSearchResult>({
				serverId,
				worldId: serverId,
				install: async (plans) => {
					const addons = await resolveQueuedAddonPlans(plans)
					const { id: coreId } = await client.cores.coreOf(serverId)
					await addServerVersions(
						client,
						coreId,
						serverId,
						addons.map((addon) => addon.version_id),
					)
				},
				onQueueChange: (plans) => setStoredServerInstallPlans(serverId, plans),
			})

			if (!result.ok) {
				handleError(result.error as Error)
				return false
			}

			queuedInstallProgress.value = {
				completed: result.flushedPlans.length,
				total: result.flushedPlans.length,
			}
			serverContentProjectIds.value = new Set([
				...serverContentProjectIds.value,
				...result.flushedPlans.map((plan) => plan.projectId),
			])
			if (result.flushedPlans.length > 0) {
				await queryClient.invalidateQueries({ queryKey: serverKeys.content(serverId) })
			}

			return true
		} finally {
			isInstallingQueuedServerInstalls.value = false
			queuedInstallProgress.value = { completed: 0, total: 0 }
		}
	}

	async function discardQueuedServerInstallsAndBack() {
		clearQueuedServerInstalls()
		await router.push(serverBackUrl.value)
	}

	async function installQueuedServerInstallsAndBack() {
		const sid = serverIdQuery.value
		const backUrl = serverBackUrl.value
		const plans = new Map(queuedServerInstalls.value)

		if (sid) writeStoredServerInstallQueue(sid, sid, plans)
		const installed = await flushQueuedServerInstalls(sid)
		if (!installed) return false
		await router.push(backUrl)

		return true
	}

	function getQueuedServerInstallPlans() {
		return queuedServerInstalls.value
	}

	function setQueuedServerInstallPlans(
		plans: Map<string, BrowseInstallPlan<InstallableSearchResult>>,
	) {
		queuedServerInstalls.value = plans
		writeStoredServerInstallQueue(serverIdQuery.value, serverIdQuery.value, plans)
	}

	function onServerFlowBack() {
		serverSetupModalRef.value?.hide()
	}

	async function handleServerModpackFlowCreate(config: CreationFlowContextValue) {
		const sid = serverIdQuery.value
		const selection = config.modpackSelection.value
		if (!sid || (!selection && !config.projectInstall.value)) {
			config.loading.value = false
			return
		}

		try {
			const { id: coreId } = await client.cores.coreOf(sid)
			if (config.projectInstall.value) {
				await config.installServerContent(coreId, sid)
			} else if (selection) {
				await setUpServer(
					client,
					coreId,
					sid,
					{ type: 'modpack', projectId: selection.projectId, versionId: selection.versionId },
					{ properties: config.buildProperties() },
				)
			}

			serverSetupModalRef.value?.hide()
			await router.push(`/hosting/manage/${sid}?openSettings=installation`)
		} catch (err) {
			handleError(err as Error)
			config.loading.value = false
		}
	}

	function markServerProjectInstalled(id: string) {
		serverContentProjectIds.value = new Set([...serverContentProjectIds.value, id])
	}

	return {
		serverIdQuery,
		browseFrom,
		serverFlowFrom,
		isFromWorlds,
		isServerContext,
		isSetupServerContext,
		serverContextServerData,
		serverContextPlatform,
		serverContentProjectIds,
		queuedServerInstallRootProjectIds,
		queuedServerInstallProjectIds,
		queuedServerInstallCount,
		selectedServerInstallProjects,
		isInstallingQueuedServerInstalls,
		queuedInstallProgress,
		serverBackUrl,
		serverBackLabel,
		serverBrowseHeading,
		clearQueuedServerInstalls,
		removeQueuedServerInstall,
		flushQueuedServerInstalls,
		discardQueuedServerInstallsAndBack,
		installQueuedServerInstallsAndBack,
		initServerContext,
		watchServerContextChanges,
		getServerProjectVersions,
		getServerUpdateChannel,
		enforceServerSetupRoute,
		getQueuedServerInstallPlans,
		setQueuedServerInstallPlans,
		resolveQueuedServerInstallPlan,
		activeServerModpackInstallProjectId,
		openServerModpackInstallFlow,
		onServerFlowHide,
		onServerFlowBack,
		handleServerModpackFlowCreate,
		markServerProjectInstalled,
	}
}
