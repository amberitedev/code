<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import {
	CheckIcon,
	CompassIcon,
	ExternalIcon,
	GlobeIcon,
	PlusIcon,
	ServerStackIcon,
	SpinnerIcon,
} from '@modrinth/assets'
import type { BrowseInstallContentType, CardAction, ProjectType, Tags } from '@modrinth/ui'
import {
	BrowsePageLayout,
	BrowseSidebar,
	commonMessages,
	ContextMenu,
	CreationFlowModal,
	defineMessages,
	formatProjectTypeSentence,
	getLatestMatchingInstallVersion,
	getSelectedInstallPreferences,
	getTargetInstallPreferences,
	injectNotificationManager,
	preferencesDiffer,
	provideBrowseManager,
	requestInstall,
	resolveInstallPlan,
	stripServerRuntimeInstallFilters,
	stripServerRuntimeInstallOverrides,
	useBrowseSearch,
	useDebugLogger,
	useVIntl,
} from '@modrinth/ui'
import { queryOptions, useQuery, useQueryClient } from '@tanstack/vue-query'
import type { Ref } from 'vue'
import { computed, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import type { LocationQuery } from 'vue-router'
import { useRoute, useRouter } from 'vue-router'

import { useAppServerBrowse } from '@/composables/browse/use-app-server-browse'
import { useAppEvent } from '@/composables/use-app-event'
import { useAppSettings } from '@/composables/use-app-settings.ts'
import { get_project, get_search_results_v3, get_version_many } from '@/helpers/cache.js'
import {
	get_installed_project_ids as getInstalledProjectIds,
	getInstanceIconUrl,
	list as listInstances,
} from '@/helpers/instance'
import { get_loader_versions as getLoaderManifest } from '@/helpers/metadata'
import { get as getSettings, set as setSettings } from '@/helpers/settings.ts'
import { get_categories, get_game_versions, get_loaders } from '@/helpers/tags'
import { get_instance_worlds } from '@/helpers/worlds'
import {
	instanceDetailQueryOptions,
	instanceKeys,
	instanceLinkedProjectQueryOptions,
} from '@/pages/instance/query-options'
import { type BreadcrumbDefinition, injectBreadcrumbManager } from '@/providers/breadcrumbs'
import { injectContentInstall } from '@/providers/content-install'
import { injectServerInstall } from '@/providers/server-install'
import {
	createServerInstallContent,
	provideServerInstallContent,
} from '@/providers/setup/server-install-content'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const { installingServerProjects, playServerProject, showAddServerToInstanceModal } =
	injectServerInstall()
const { install: installVersion } = injectContentInstall()
const queryClient = useQueryClient()
const debugLog = useDebugLogger('Browse')

const router = useRouter()
const route = useRoute()
const displayedBrowseRoute = shallowRef(router.currentRoute.value)
watch(
	() => router.currentRoute.value,
	(nextRoute) => {
		if (nextRoute.path.startsWith('/browse/')) {
			displayedBrowseRoute.value = nextRoute
		}
	},
	{ immediate: true },
)
const breadcrumbMessages = defineMessages({
	discoverProjectType: {
		id: 'app.browse.discover-project-type',
		defaultMessage: 'Discover {projectType}',
	},
	discoverServers: {
		id: 'app.browse.discover-servers',
		defaultMessage: 'Discover servers',
	},
})
const breadcrumbLabel = computed(() => {
	const browseRoute = displayedBrowseRoute.value
	if (browseRoute.query.from === 'worlds' || browseRoute.params.projectType === 'server') {
		return formatMessage(breadcrumbMessages.discoverServers)
	}

	return formatMessage(breadcrumbMessages.discoverProjectType, {
		projectType: formatProjectTypeSentence(
			formatMessage,
			String(browseRoute.params.projectType ?? ''),
			2,
		),
	})
})
const appSettings = useAppSettings()
const browseRouteActive = computed(() => route.path.startsWith('/browse/'))
const serverSetupModalRef = ref<InstanceType<typeof CreationFlowModal> | null>(null)
const serverInstallContent = createServerInstallContent({ serverSetupModalRef })
provideServerInstallContent(serverInstallContent)
const {
	serverIdQuery,
	serverFlowFrom,
	isFromWorlds,
	isServerContext,
	isSetupServerContext,
	effectiveServerWorldId,
	serverContextServerData,
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
	searchServerModpacks,
	getServerProjectVersions,
	enforceSetupModpackRoute,
	getQueuedServerInstallPlans,
	setQueuedServerInstallPlans,
	resolveQueuedServerInstallPlan,
	openServerModpackInstallFlow,
	onServerFlowBack,
	handleServerModpackFlowCreate,
	markServerProjectInstalled,
} = serverInstallContent

const initialInstanceId = computed(() => String(route.query.i ?? ''))
const instanceQuery = useQuery(
	computed(() =>
		queryOptions({
			...instanceDetailQueryOptions(initialInstanceId.value),
			enabled: !!initialInstanceId.value,
		}),
	),
)
const instance = computed(() => instanceQuery.data.value ?? null)
const linkedInstanceProjectId = computed(() => instance.value?.link?.project_id ?? '')
const linkedInstanceProjectQuery = useQuery(
	computed(() =>
		queryOptions({
			...instanceLinkedProjectQueryOptions(linkedInstanceProjectId.value),
			enabled: !!linkedInstanceProjectId.value,
		}),
	),
)
// Starts from the last read, so a return visit can search before the fresh read arrives.
const installedProjectIds: Ref<string[] | null> = ref(
	queryClient.getQueryData<string[]>(installedProjectIdsQuery().queryKey) ?? null,
)
const instanceHideInstalled = ref(route.query.ai === 'true')
const newlyInstalled = ref<string[]>([])
const hiddenInstanceProjectIds = ref<Set<string>>(new Set())
const hiddenInstanceProjectIdsInitialized = ref(false)
const isServerInstance = computed(
	() => linkedInstanceProjectQuery.data.value?.minecraft_server != null,
)

const breadcrumbManager = injectBreadcrumbManager()
const instanceBreadcrumbDefinition = {
	slot: 'instance',
	id: () => `instance:${String(displayedBrowseRoute.value.query.i ?? '')}`,
	label: () => instance.value?.name ?? formatMessage(commonMessages.loadingLabel),
	visual: () => ({
		type: 'image' as const,
		src: getInstanceIconUrl(instance.value?.icon_path),
		alt: instance.value?.name,
		tintBy: String(displayedBrowseRoute.value.query.i ?? ''),
	}),
	to: () => {
		const instancePath = `/instance/${encodeURIComponent(
			String(displayedBrowseRoute.value.query.i ?? ''),
		)}`
		return displayedBrowseRoute.value.query.from === 'worlds'
			? `${instancePath}/worlds`
			: instancePath
	},
} satisfies BreadcrumbDefinition
const serversBreadcrumbDefinition = {
	slot: 'root',
	id: 'servers',
	label: () => formatMessage(commonMessages.serversLabel),
	to: '/hosting/manage/',
	visual: { type: 'icon', component: ServerStackIcon },
} satisfies BreadcrumbDefinition
const serverBreadcrumbTo = ref(serverBackUrl.value)
watch(serverBackUrl, (value) => {
	if (route.path.startsWith('/browse/')) {
		serverBreadcrumbTo.value = value
	}
})
const serverBreadcrumbDefinition = {
	slot: 'server',
	id: () => `server:${String(displayedBrowseRoute.value.query.sid ?? '')}`,
	label: () => serverContextServerData.value?.name ?? formatMessage(commonMessages.loadingLabel),
	visual: { type: 'icon', component: ServerStackIcon },
	to: serverBreadcrumbTo,
} satisfies BreadcrumbDefinition
const breadcrumbDefinition = {
	slot: 'browse',
	id: () =>
		`browse:${String(displayedBrowseRoute.value.params.projectType ?? '')}:${String(
			displayedBrowseRoute.value.query.i ?? '',
		)}:${String(displayedBrowseRoute.value.query.sid ?? '')}:${String(
			displayedBrowseRoute.value.query.from ?? '',
		)}`,
	label: breadcrumbLabel,
	to: () => displayedBrowseRoute.value.fullPath,
	visual: { type: 'icon', component: CompassIcon },
} satisfies BreadcrumbDefinition

function syncBreadcrumbs() {
	if (displayedBrowseRoute.value.query.i) {
		const instanceBreadcrumb = breadcrumbManager.reset(instanceBreadcrumbDefinition)
		breadcrumbManager.push(breadcrumbDefinition, { parent: instanceBreadcrumb })
		return
	}

	if (displayedBrowseRoute.value.query.sid) {
		const serversBreadcrumb = breadcrumbManager.reset(serversBreadcrumbDefinition)
		const serverBreadcrumb = breadcrumbManager.push(serverBreadcrumbDefinition, {
			parent: serversBreadcrumb,
		})
		breadcrumbManager.push(breadcrumbDefinition, { parent: serverBreadcrumb })
		return
	}

	breadcrumbManager.reset(breadcrumbDefinition)
}

watch(displayedBrowseRoute, syncBreadcrumbs, { immediate: true, flush: 'sync' })

debugLog('fetching tags (categories, loaders, gameVersions)')
// Tags do not change while the App runs, so only the first Browse visit waits for them.
const tagQueryOptions = { staleTime: Infinity, gcTime: Infinity } as const
const [categories, loaders, availableGameVersions] = await Promise.all([
	queryClient
		.ensureQueryData({
			queryKey: ['tags', 'categories'],
			queryFn: get_categories,
			...tagQueryOptions,
		})
		.catch(handleError)
		.then(ref<Labrinth.Tags.v2.Category[]>),
	queryClient
		.ensureQueryData({ queryKey: ['tags', 'loaders'], queryFn: get_loaders, ...tagQueryOptions })
		.catch(handleError)
		.then(ref<Labrinth.Tags.v2.Loader[]>),
	queryClient
		.ensureQueryData({
			queryKey: ['tags', 'game-versions'],
			queryFn: get_game_versions,
			...tagQueryOptions,
		})
		.catch(handleError)
		.then(ref<Labrinth.Tags.v2.GameVersion[]>),
])

const tags: Ref<Tags> = computed(() => ({
	gameVersions: availableGameVersions.value ?? [],
	loaders: loaders.value ?? [],
	categories: categories.value ?? [],
}))

if (isFromWorlds.value && route.params.projectType !== 'server') {
	router.replace({
		path: '/browse/server',
		query: route.query,
	})
}

enforceSetupModpackRoute(route.params.projectType as string | undefined)

const allInstalledIds = computed(
	() => new Set([...newlyInstalled.value, ...(installedProjectIds.value ?? [])]),
)

function syncHiddenInstanceProjectIds() {
	hiddenInstanceProjectIds.value = new Set([
		...(installedProjectIds.value ?? []),
		...newlyInstalled.value,
	])
	hiddenInstanceProjectIdsInitialized.value = true
}

watch(
	installedProjectIds,
	(ids) => {
		if (!ids) return
		if (!hiddenInstanceProjectIdsInitialized.value) {
			syncHiddenInstanceProjectIds()
		}
	},
	{ immediate: true },
)

watchServerContextChanges()

// True when everything the first search depends on is already cached. The search then runs during
// setup and its cached results are on screen in the first frame. Otherwise it waits for the context
// below, with the placeholders showing.
const searchContextReady = ref(
	installedProjectIds.value !== null &&
		!route.query.sid &&
		(!route.query.i ||
			(!!instance.value &&
				(!linkedInstanceProjectId.value || linkedInstanceProjectQuery.data.value !== undefined))),
)

// Not awaited, so entering Browse does not hold the previous page on screen.
void initInstanceContext().finally(() => {
	// The hidden ids were first taken from cached or still empty state, so take them again from
	// what was just loaded, as a first visit does.
	if (installedProjectIds.value) syncHiddenInstanceProjectIds()
	syncHiddenServerContentProjectIds()
	addInstanceGameVersionToServerFilters()
	searchContextReady.value = true
})

// The ids of what is already installed, for the context Browse was opened in.
function installedProjectIdsQuery(): {
	queryKey: readonly unknown[]
	queryFn: () => Promise<string[]>
	staleTime: number
} {
	const targetInstanceId = route.query.i ? String(route.query.i) : null

	if (!targetInstanceId) {
		return {
			queryKey: [...instanceKeys.all, 'installed-project-ids'],
			queryFn: async () =>
				(await listInstances())
					.map((gameInstance) => gameInstance.link?.project_id)
					.filter((id): id is string => !!id),
			staleTime: 0,
		}
	}

	if (route.query.from === 'worlds') {
		return {
			queryKey: instanceKeys.installedProjectIds(targetInstanceId, 'worlds'),
			queryFn: async () =>
				(await get_instance_worlds(targetInstanceId))
					.filter((w) => w.type === 'server' && 'project_id' in w && w.project_id)
					.map((w) => (w as { project_id: string }).project_id),
			staleTime: 0,
		}
	}

	return {
		queryKey: instanceKeys.installedProjectIds(targetInstanceId, 'content'),
		queryFn: () => getInstalledProjectIds(targetInstanceId),
		staleTime: 0,
	}
}

async function refreshInstalledProjectIds() {
	const ids = await queryClient.fetchQuery(installedProjectIdsQuery()).catch(handleError)
	if (!ids) return

	debugLog('installedProjectIds loaded', { count: ids.length })
	installedProjectIds.value = ids
}

async function initInstanceContext() {
	debugLog('initInstanceContext', {
		queryI: route.query.i,
		queryAi: route.query.ai,
		querySid: route.query.sid,
		queryWid: route.query.wid,
		queryFrom: route.query.from,
	})
	await Promise.all([
		initServerContext(),
		refreshInstalledProjectIds(),
		route.query.i ? instanceQuery.suspense().catch(handleError) : Promise.resolve(),
	])

	if (route.query.i) {
		debugLog('instance loaded', {
			name: instance.value?.name,
			loader: instance.value?.loader,
			gameVersion: instance.value?.game_version,
		})

		if (instance.value?.link?.project_id) {
			await linkedInstanceProjectQuery.suspense().catch(handleError)
		}
	}
}

function setHideInstalledModpacks(value: boolean) {
	appSettings.hideInstalledModpacks = value
	getSettings()
		.then((settings) => {
			settings.hide_installed_modpacks = value
			return setSettings(settings)
		})
		.catch(handleError)
}

const hideInstalledModpacks = computed({
	get: () => appSettings.hideInstalledModpacks,
	set: (value: boolean) => setHideInstalledModpacks(value),
})

// Take the project type so a tab's search can be built before switching to it.
function getInstanceFilters(pt: ProjectType) {
	const filters = []

	if (instance.value && pt !== 'resourcepack') {
		const isVanillaShader = pt === 'shader' && instance.value.loader === 'vanilla'
		const gameVersion = instance.value.game_version
		if (gameVersion && !isVanillaShader) {
			filters.push({ type: 'game_version', option: gameVersion })
		}

		const platform = instance.value.loader
		const supportedModLoaders = ['fabric', 'forge', 'quilt', 'neoforge']

		if (platform && pt === 'mod' && supportedModLoaders.includes(platform)) {
			filters.push({ type: 'mod_loader', option: platform })
		}
		if (isVanillaShader) {
			filters.push({ type: 'shader_loader', option: 'vanilla' })
		}

		if (isServerInstance.value) {
			filters.push({ type: 'environment', option: 'client' })
		}
	}

	if (
		(instance.value || pt === 'modpack') &&
		(pt === 'modpack' ? hideInstalledModpacks.value : instanceHideInstalled.value) &&
		hiddenInstanceProjectIds.value.size > 0
	) {
		for (const id of hiddenInstanceProjectIds.value) {
			filters.push({ type: 'project_id', option: `project_id:${id}`, negative: true })
		}
	}

	return filters
}

const serverHideInstalled = ref(false)
const hideSelectedServerInstalls = ref(false)
if (route.query.shi) {
	serverHideInstalled.value = route.query.shi === 'true'
}
const hiddenServerContentProjectIds = ref<Set<string>>(new Set())
const hiddenServerContentProjectIdsInitialized = ref(false)

function syncHiddenServerContentProjectIds() {
	hiddenServerContentProjectIds.value = new Set(serverContentProjectIds.value)
	hiddenServerContentProjectIdsInitialized.value = true
}

watch(
	serverContentProjectIds,
	() => {
		if (!hiddenServerContentProjectIdsInitialized.value) {
			syncHiddenServerContentProjectIds()
		}
	},
	{ immediate: true },
)

function getServerContextFilters(pt: ProjectType) {
	const filters: { type: string; option: string; negative?: boolean }[] = []
	if (!serverContextServerData.value) return filters

	if (pt !== 'modpack') {
		const gameVersion = serverContextServerData.value.mc_version
		if (gameVersion) filters.push({ type: 'game_version', option: gameVersion })

		const platform = serverContextServerData.value.loader?.toLowerCase()
		if (platform && ['fabric', 'forge', 'quilt', 'neoforge'].includes(platform))
			filters.push({ type: 'mod_loader', option: platform })
		if (platform && ['paper', 'purpur'].includes(platform))
			filters.push({ type: 'plugin_loader', option: platform })

		if (pt === 'mod') filters.push({ type: 'environment', option: 'server' })

		if (hideSelectedServerInstalls.value && queuedServerInstallProjectIds.value.size > 0) {
			for (const id of queuedServerInstallProjectIds.value) {
				filters.push({ type: 'project_id', option: `project_id:${id}`, negative: true })
			}
		}
	}

	if (pt === 'modpack') {
		filters.push(
			{ type: 'environment', option: 'client' },
			{ type: 'environment', option: 'server' },
		)

		if (hideInstalledModpacks.value && hiddenInstanceProjectIds.value.size > 0) {
			for (const id of hiddenInstanceProjectIds.value) {
				filters.push({ type: 'project_id', option: `project_id:${id}`, negative: true })
			}
		}
	}

	if (serverHideInstalled.value && hiddenServerContentProjectIds.value.size > 0) {
		for (const id of hiddenServerContentProjectIds.value) {
			filters.push({ type: 'project_id', option: `project_id:${id}`, negative: true })
		}
	}

	return filters
}

function getProvidedFilters(pt: ProjectType) {
	return isServerContext.value ? getServerContextFilters(pt) : getInstanceFilters(pt)
}

const combinedProvidedFilters = computed(() => getProvidedFilters(projectType.value))

const {
	serverPings,
	contextMenuRef,
	updateServerHits,
	getServerModpackContent,
	getServerCardActions,
	handleRightClick,
} = useAppServerBrowse({
	instance,
	isFromWorlds,
	allInstalledIds,
	newlyInstalled,
	installingServerProjects,
	playServerProject,
	showAddServerToInstanceModal,
	handleError,
	router,
})

const offline = ref(!navigator.onLine)
const handleOffline = () => {
	debugLog('went offline')
	offline.value = true
}
const handleOnline = () => {
	debugLog('went online')
	offline.value = false
}
window.addEventListener('offline', handleOffline)
window.addEventListener('online', handleOnline)

onBeforeUnmount(() => {
	window.removeEventListener('offline', handleOffline)
	window.removeEventListener('online', handleOnline)
})

const messages = defineMessages({
	addServersToInstance: {
		id: 'app.browse.add-servers-to-instance',
		defaultMessage: 'Adding server to instance',
	},
	projectActionsLabel: {
		id: 'app.browse.project-actions.label',
		defaultMessage: 'Project actions',
	},
	addToAnInstance: {
		id: 'app.browse.add-to-an-instance',
		defaultMessage: 'Add to an instance',
	},
	environmentProvidedByServer: {
		id: 'search.filter.locked.server-environment.title',
		defaultMessage: 'Only client-side mods can be added to the server instance',
	},
	environmentProvidedByCoreServer: {
		id: 'app.browse.filter.locked.core-server-environment.title',
		defaultMessage: 'Only mods compatible with servers can be added to this server',
	},
	gameVersionProvidedByInstance: {
		id: 'search.filter.locked.instance-game-version.title',
		defaultMessage: 'Game version is provided by the instance',
	},
	hideAddedServers: {
		id: 'app.browse.hide-added-servers',
		defaultMessage: 'Hide servers already added',
	},
	hideInstalledModpacks: {
		id: 'app.browse.hide-installed-modpacks',
		defaultMessage: 'Hide already installed',
	},
	installingToServer: {
		id: 'app.browse.server.installing',
		defaultMessage: 'Installing',
	},
	backToInstance: {
		id: 'app.browse.back-to-instance',
		defaultMessage: 'Back to instance',
	},
	serverInstanceContentWarning: {
		id: 'app.browse.server-instance-content-warning',
		defaultMessage:
			'Adding content may prevent you from joining this server. Any content you add will be removed when the managed server content is updated.',
	},
	modLoaderProvidedByInstance: {
		id: 'search.filter.locked.instance-loader.title',
		defaultMessage: 'Loader is provided by the instance',
	},
	modpacksProjectType: {
		id: 'app.browse.project-type.modpacks',
		defaultMessage: 'Modpacks',
	},
	modsProjectType: { id: 'app.browse.project-type.mods', defaultMessage: 'Mods' },
	resourcePacksProjectType: {
		id: 'app.browse.project-type.resource-packs',
		defaultMessage: 'Resource Packs',
	},
	dataPacksProjectType: {
		id: 'app.browse.project-type.data-packs',
		defaultMessage: 'Data Packs',
	},
	shadersProjectType: { id: 'app.browse.project-type.shaders', defaultMessage: 'Shaders' },
	serversProjectType: { id: 'app.browse.project-type.servers', defaultMessage: 'Servers' },
	providedByInstance: {
		id: 'search.filter.locked.instance',
		defaultMessage: 'Provided by the instance',
	},
	syncFilterButton: {
		id: 'search.filter.locked.instance.sync',
		defaultMessage: 'Sync with instance',
	},
})

const projectType = ref<ProjectType>(route.params.projectType as ProjectType)

function resetInstanceContext() {
	debugLog('instance context removed, resetting')
	installedProjectIds.value = null
	instanceHideInstalled.value = false
	newlyInstalled.value = []
	hiddenInstanceProjectIds.value = new Set()
	hiddenInstanceProjectIdsInitialized.value = false
	void refreshInstalledProjectIds()
}

watch(
	() => route.params.projectType as ProjectType,
	async (newType) => {
		if (!browseRouteActive.value) {
			return
		}
		if (isSetupServerContext.value) {
			enforceSetupModpackRoute(newType)
			if (newType !== 'modpack') return
		}

		if (!newType || newType === projectType.value) return

		debugLog('projectType route param changed', { from: projectType.value, to: newType })
		projectType.value = newType
	},
)

watch(
	() => route.query.i,
	async (nextInstanceId, previousInstanceId) => {
		if (!route.path.startsWith('/browse') || nextInstanceId === previousInstanceId) return
		if (!nextInstanceId) {
			resetInstanceContext()
			return
		}

		installedProjectIds.value = null
		newlyInstalled.value = []
		hiddenInstanceProjectIds.value = new Set()
		hiddenInstanceProjectIdsInitialized.value = false
		await Promise.all([instanceQuery.suspense().catch(handleError), refreshInstalledProjectIds()])
		if (instance.value?.link?.project_id) {
			await linkedInstanceProjectQuery.suspense().catch(handleError)
		}
	},
)

// A string, so the tabs below are only rebuilt when it changes and not on every search, which
// replaces the route query.
const projectTypeTabSuffix = computed(() => {
	const params: LocationQuery = {}

	if (route.query.i) params.i = route.query.i
	if (route.query.ai) params.ai = route.query.ai
	if (route.query.from) params.from = route.query.from
	if (route.query.sid) params.sid = route.query.sid
	if (effectiveServerWorldId.value) params.wid = effectiveServerWorldId.value

	const queryString = new URLSearchParams(params as Record<string, string>).toString()
	return queryString ? `?${queryString}` : ''
})

const selectableProjectTypes = computed(() => {
	let dataPacks = false,
		mods = false,
		modpacks = false

	if (instance.value) {
		if (
			availableGameVersions.value &&
			availableGameVersions.value.findIndex((x) => x.version === instance.value?.game_version) <=
				availableGameVersions.value.findIndex((x) => x.version === '1.13') &&
			!isServerInstance.value
		) {
			dataPacks = true
		}

		if (instance.value.loader !== 'vanilla') {
			mods = true
		}
	} else {
		dataPacks = true
		mods = true
		modpacks = true
	}

	const suffix = projectTypeTabSuffix.value
	const prefetch = (type: ProjectType) => () => searchState.prefetchProjectType(type)

	if (isSetupServerContext.value) {
		return [
			{ label: formatMessage(messages.modpacksProjectType), href: `/browse/modpack${suffix}` },
		]
	}

	if (isFromWorlds.value) {
		return [{ label: formatMessage(messages.serversProjectType), href: `/browse/server${suffix}` }]
	}

	return [
		{
			label: formatMessage(messages.modpacksProjectType),
			href: `/browse/modpack${suffix}`,
			shown: modpacks,
			onHover: prefetch('modpack'),
		},
		{
			label: formatMessage(messages.modsProjectType),
			href: `/browse/mod${suffix}`,
			shown: mods,
			onHover: prefetch('mod'),
		},
		{
			label: formatMessage(messages.resourcePacksProjectType),
			href: `/browse/resourcepack${suffix}`,
			onHover: prefetch('resourcepack'),
		},
		{
			label: formatMessage(messages.dataPacksProjectType),
			href: `/browse/datapack${suffix}`,
			shown: dataPacks,
			onHover: prefetch('datapack'),
		},
		{
			label: formatMessage(messages.shadersProjectType),
			href: `/browse/shader${suffix}`,
			onHover: prefetch('shader'),
		},
		{
			label: formatMessage(messages.serversProjectType),
			href: `/browse/server${suffix}`,
			shown: !instance.value,
			onHover: prefetch('server'),
		},
	]
})

const installContext = computed(() => {
	if (isServerContext.value && serverContextServerData.value) {
		return {
			name: serverContextServerData.value.name,
			loader: serverContextServerData.value.loader ?? '',
			gameVersion: serverContextServerData.value.mc_version ?? '',
			serverId: serverIdQuery.value,
			upstream: serverContextServerData.value.upstream,
			iconSrc: null as string | null,
			isMedal: serverContextServerData.value.is_medal,
			backUrl: serverBackUrl.value,
			backLabel: serverBackLabel.value,
			heading: serverBrowseHeading.value,
			queuedCount: queuedServerInstallCount.value,
			selectedProjects: selectedServerInstallProjects.value,
			isInstallingSelected: isInstallingQueuedServerInstalls.value,
			skipNonEssentialWarnings: appSettings.skipNonEssentialWarnings,
			installProgress: queuedInstallProgress.value,
			clearQueued: clearQueuedServerInstalls,
			clearSelected: clearQueuedServerInstalls,
			onBack: flushQueuedServerInstalls,
			discardSelectedAndBack: discardQueuedServerInstallsAndBack,
			installSelected: installQueuedServerInstallsAndBack,
		}
	}
	if (instance.value) {
		return {
			name: instance.value.name,
			loader: instance.value.loader,
			gameVersion: instance.value.game_version,
			iconSrc: getInstanceIconUrl(instance.value.icon_path),
			backUrl: `/instance/${encodeURIComponent(instance.value.id)}${isFromWorlds.value ? '/worlds' : ''}`,
			backLabel: formatMessage(messages.backToInstance),
			heading: formatMessage(
				isFromWorlds.value ? messages.addServersToInstance : commonMessages.installingContentLabel,
			),
			warning:
				isServerInstance.value && instance.value.loader !== 'vanilla' && !isFromWorlds.value
					? formatMessage(messages.serverInstanceContentWarning)
					: undefined,
		}
	}
	return null
})

const installingProjectIds = ref<Set<string>>(new Set())

function setProjectInstalling(projectId: string, installing: boolean) {
	const next = new Set(installingProjectIds.value)
	if (installing) {
		next.add(projectId)
	} else {
		next.delete(projectId)
	}
	installingProjectIds.value = next
}

const serverInstallQueue = {
	get: getQueuedServerInstallPlans,
	set: setQueuedServerInstallPlans,
}

function getCurrentSelectedInstallPreferences(projectTypeValue: string) {
	return getSelectedInstallPreferences({
		contentType: projectTypeValue,
		selectedFilters: searchState.currentFilters.value,
		providedFilters: combinedProvidedFilters.value,
		overriddenProvidedFilterTypes: searchState.overriddenProvidedFilterTypes.value,
	})
}

function getServerInstallTargetPreferences(contentType: BrowseInstallContentType) {
	return getTargetInstallPreferences(
		{
			gameVersion: serverContextServerData.value?.mc_version,
			loader: serverContextServerData.value?.loader,
		},
		contentType,
	)
}

function getInstanceInstallTargetPreferences(projectTypeValue: string) {
	return getTargetInstallPreferences(
		{
			gameVersion: instance.value?.game_version,
			loader: instance.value?.loader,
		},
		projectTypeValue,
	)
}

async function getInstallProjectVersions(projectId: string) {
	const project = await get_project(projectId, 'must_revalidate')
	return (await get_version_many(
		project.versions,
		'must_revalidate',
	)) as Labrinth.Versions.v2.Version[]
}

async function chooseInstanceInstallVersion(
	project: Labrinth.Search.v3.ResultSearchProject,
	projectTypeValue: string,
) {
	const targetInstance = instance.value
	if (!targetInstance) {
		return { versionId: null as string | null }
	}

	const selectedPreferences = getCurrentSelectedInstallPreferences(projectTypeValue)
	const targetPreferences = getInstanceInstallTargetPreferences(projectTypeValue)
	if (!preferencesDiffer(selectedPreferences, targetPreferences)) {
		return { versionId: null as string | null }
	}

	const selectedVersion = getLatestMatchingInstallVersion(
		await getInstallProjectVersions(project.project_id),
		selectedPreferences,
	)

	if (!selectedVersion) {
		return { versionId: null as string | null }
	}

	return { versionId: selectedVersion.id }
}

async function chooseFilterMatchingInstallVersion(
	project: Labrinth.Search.v3.ResultSearchProject,
	projectTypeValue: string,
) {
	const plan = await resolveInstallPlan({
		project: {
			project_id: project.project_id,
			// @ts-expect-error v3 search results carry name, not title
			title: project.title,
			icon_url: project.icon_url,
		},
		contentType: projectTypeValue as BrowseInstallContentType,
		selectedFilters: searchState.currentFilters.value,
		providedFilters: combinedProvidedFilters.value,
		overriddenProvidedFilterTypes: searchState.overriddenProvidedFilterTypes.value,
		targetPreferences: {},
		getProjectVersions: getInstallProjectVersions,
	})

	return { versionId: plan.versionId }
}

function getCardActions(
	result: Labrinth.Search.v3.ResultSearchProject,
	currentProjectType: string,
): CardAction[] {
	if (currentProjectType === 'server') {
		return getServerCardActions(result)
	}

	const projectResult = result
	const isInstalled =
		allInstalledIds.value.has(projectResult.project_id || '') ||
		serverContentProjectIds.value.has(projectResult.project_id || '') ||
		serverContextServerData.value?.upstream?.project_id === projectResult.project_id
	const isInstalling = installingProjectIds.value.has(projectResult.project_id)
	const showAsInstalled = isInstalled && currentProjectType !== 'modpack'

	if (
		isServerContext.value &&
		['modpack', 'mod', 'plugin', 'datapack'].includes(currentProjectType)
	) {
		const isQueued = queuedServerInstallProjectIds.value.has(projectResult.project_id)
		const isQueuedRoot = queuedServerInstallRootProjectIds.value.has(projectResult.project_id)
		const isInstallingSelection = isInstallingQueuedServerInstalls.value
		const validatingInstall =
			isInstalling && currentProjectType !== 'modpack' && !isInstallingSelection
		const installLabel = showAsInstalled
			? commonMessages.installedLabel
			: isQueued
				? isInstalling || isInstallingSelection
					? validatingInstall
						? commonMessages.validatingLabel
						: messages.installingToServer
					: commonMessages.selectedLabel
				: isInstalling || isInstallingSelection
					? validatingInstall
						? commonMessages.validatingLabel
						: messages.installingToServer
					: commonMessages.installButton
		return [
			{
				key: 'install',
				label: formatMessage(installLabel),
				icon:
					isInstalling || isInstallingSelection
						? SpinnerIcon
						: isQueued || showAsInstalled
							? CheckIcon
							: PlusIcon,
				iconClass: isInstalling || isInstallingSelection ? 'animate-spin' : undefined,
				disabled:
					showAsInstalled || isInstalling || isInstallingSelection || (isQueued && !isQueuedRoot),
				color: isQueued && !isInstalling && !isInstallingSelection ? 'green' : 'brand',
				type: 'outlined',
				onClick: async () => {
					if (isQueuedRoot) {
						removeQueuedServerInstall(projectResult.project_id)
						return
					}
					if (isQueued) return

					const contentType = currentProjectType as BrowseInstallContentType
					const isModpack = contentType === 'modpack'
					const shouldShowInstalling = isModpack || !isQueued
					if (shouldShowInstalling) {
						setProjectInstalling(projectResult.project_id, true)
					}
					try {
						const plan = await requestInstall({
							project: projectResult,
							contentType,
							mode: isModpack ? 'immediate' : 'queue',
							selectedFilters: isModpack
								? []
								: stripServerRuntimeInstallFilters(searchState.currentFilters.value),
							providedFilters: isModpack ? [] : combinedProvidedFilters.value,
							overriddenProvidedFilterTypes: isModpack
								? []
								: stripServerRuntimeInstallOverrides(
										searchState.overriddenProvidedFilterTypes.value,
									),
							targetPreferences: getServerInstallTargetPreferences(contentType),
							getProjectVersions: getInstallProjectVersions,
							queue: serverInstallQueue,
							install: (plan) =>
								openServerModpackInstallFlow({
									projectId: plan.projectId,
									versionId: plan.versionId,
									name: plan.project.name,
									iconUrl: plan.project.icon_url ?? undefined,
								}),
						})
						if (!isModpack) await resolveQueuedServerInstallPlan(plan)
					} catch (err) {
						if (!isModpack) removeQueuedServerInstall(projectResult.project_id)
						handleError(err as Error)
					} finally {
						if (shouldShowInstalling) {
							setProjectInstalling(projectResult.project_id, false)
						}
					}
				},
			},
		]
	}

	const isModpack = projectResult.project_types?.includes('modpack')
	const shouldUseInstallIcon = !!instance.value || isModpack

	return [
		{
			key: 'install',
			label: formatMessage(
				isInstalling
					? messages.installingToServer
					: showAsInstalled
						? commonMessages.installedLabel
						: shouldUseInstallIcon
							? commonMessages.installButton
							: messages.addToAnInstance,
			),
			icon: isInstalling ? SpinnerIcon : showAsInstalled ? CheckIcon : PlusIcon,
			iconClass: isInstalling ? 'animate-spin' : undefined,
			disabled: showAsInstalled || isInstalling,
			color: 'brand',
			type: 'outlined',
			onClick: async () => {
				setProjectInstalling(projectResult.project_id, true)
				try {
					const selectedInstall = instance.value
						? await chooseInstanceInstallVersion(projectResult, currentProjectType)
						: isModpack
							? await chooseFilterMatchingInstallVersion(projectResult, currentProjectType)
							: { versionId: null as string | null }
					if (selectedInstall === null) {
						setProjectInstalling(projectResult.project_id, false)
						return
					}
					const selectedPreferences = getCurrentSelectedInstallPreferences(currentProjectType)
					await installVersion(
						projectResult.project_id,
						selectedInstall.versionId,
						instance.value ? instance.value.id : null,
						'SearchCard',
						(versionId, installedProjectIds) => {
							setProjectInstalling(projectResult.project_id, false)
							if (versionId) {
								onSearchResultsInstalled(installedProjectIds ?? [projectResult.project_id])
							}
						},
						(profile) => {
							router.push(`/instance/${profile}`)
						},
						{
							preferredLoader: instance.value?.loader ?? selectedPreferences.loaders?.[0],
							preferredGameVersion:
								instance.value?.game_version ?? selectedPreferences.gameVersions?.[0],
						},
					)
				} catch (err) {
					setProjectInstalling(projectResult.project_id, false)
					handleError(err)
				}
			},
		},
	]
}

function onSearchResultInstalled(id: string) {
	if (isServerContext.value) {
		markServerProjectInstalled(id)
		return
	}
	if (!newlyInstalled.value.includes(id)) {
		newlyInstalled.value = [...newlyInstalled.value, id]
	}
}

function onSearchResultsInstalled(ids: string[]) {
	if (isServerContext.value) {
		for (const id of ids) {
			markServerProjectInstalled(id)
		}
		return
	}
	newlyInstalled.value = Array.from(new Set([...newlyInstalled.value, ...ids]))
}

type RawSearchResults = { result: Labrinth.Search.v3.SearchResults } | null

// Coming back to a search within a few minutes shows the same page at once.
const SEARCH_STALE_TIME = 5 * 60_000

function searchQueryOptions(requestParams: string) {
	return queryOptions({
		queryKey: ['search', 'v3', requestParams],
		queryFn: async () => {
			const rawResults = (await get_search_results_v3(
				requestParams,
				'must_revalidate',
			)) as RawSearchResults

			// Seeded here so it happens once per fetch, not again each time a cached search is shown.
			for (const hit of rawResults?.result.hits ?? []) {
				for (const identifier of [hit.project_id, hit.slug]) {
					if (identifier) {
						queryClient.setQueryData(['projects', 'summary', identifier], hit)
					}
				}
			}

			return rawResults
		},
		staleTime: SEARCH_STALE_TIME,
	})
}

async function search(requestParams: string) {
	debugLog('searching v3', requestParams)
	const isServer = projectType.value === 'server'
	return toSearchResponse(await queryClient.fetchQuery(searchQueryOptions(requestParams)), isServer)
}

// The cached results of a search that `search` would return without fetching.
function peekSearch(requestParams: string) {
	const state = queryClient.getQueryState<RawSearchResults>(
		searchQueryOptions(requestParams).queryKey,
	)
	if (
		!state ||
		state.data === undefined ||
		state.isInvalidated ||
		Date.now() - state.dataUpdatedAt >= SEARCH_STALE_TIME
	) {
		return undefined
	}
	return toSearchResponse(state.data, projectType.value === 'server')
}

function toSearchResponse(rawResults: RawSearchResults, isServer: boolean) {
	if (!rawResults) {
		return {
			projectHits: [],
			serverHits: [],
			total_hits: 0,
			per_page: 20,
		}
	}

	if (isServer) {
		const hits = rawResults.result.hits ?? []
		updateServerHits(hits)
		return {
			projectHits: [],
			serverHits: hits,
			total_hits: rawResults.result.total_hits ?? 0,
			per_page: rawResults.result.hits_per_page,
		}
	}

	return {
		projectHits: rawResults.result.hits,
		serverHits: [],
		total_hits: rawResults.result.total_hits,
		per_page: rawResults.result.hits_per_page,
	}
}

const lockedFilterMessages = computed(() => ({
	gameVersion: formatMessage(messages.gameVersionProvidedByInstance),
	modLoader: formatMessage(messages.modLoaderProvidedByInstance),
	environment: formatMessage(
		isServerContext.value
			? messages.environmentProvidedByCoreServer
			: messages.environmentProvidedByServer,
	),
	syncButton: formatMessage(messages.syncFilterButton),
	providedBy: formatMessage(messages.providedByInstance),
}))

const searchState = useBrowseSearch({
	projectType,
	tags,
	active: computed(() => browseRouteActive.value && searchContextReady.value),
	providedFilters: combinedProvidedFilters,
	providedFiltersFor: (type) => getProvidedFilters(type as ProjectType),
	search,
	peekSearch,
	prefetchSearch: (requestParams) =>
		void queryClient.prefetchQuery(searchQueryOptions(requestParams)),
	persistentQueryParams: ['i', 'ai', 'shi', 'sid', 'wid', 'from'],
	getExtraQueryParams: () => ({
		sid: serverIdQuery.value || undefined,
		wid: effectiveServerWorldId.value || undefined,
		ai: instanceHideInstalled.value ? 'true' : undefined,
		shi: serverHideInstalled.value ? 'true' : undefined,
	}),
})

watch(
	[
		() => searchState.query.value,
		() =>
			searchState.isServerType.value
				? searchState.serverCurrentFilters.value
				: searchState.currentFilters.value,
		() => projectType.value,
	],
	() => {
		if (isServerContext.value && projectType.value !== 'modpack') {
			syncHiddenServerContentProjectIds()
		} else if (instance.value || projectType.value === 'modpack') {
			syncHiddenInstanceProjectIds()
		}
	},
	{ deep: true },
)

watch(queuedServerInstallCount, (count) => {
	if (count === 0) {
		hideSelectedServerInstalls.value = false
	}
})

// Runs during setup when the instance is cached, and again once the context has loaded.
function addInstanceGameVersionToServerFilters() {
	const gv = instance.value?.game_version
	if (!gv) return
	const alreadyHasGv = searchState.serverCurrentFilters.value.some(
		(f) => f.type === 'server_game_version' && f.option === gv,
	)
	if (!alreadyHasGv) {
		searchState.serverCurrentFilters.value.push({ type: 'server_game_version', option: gv })
	}
}

addInstanceGameVersionToServerFilters()

// Does nothing until the search context is ready; becoming ready starts the search instead.
void searchState.refreshSearch()

useAppEvent('instance', async (event) => {
	if (event.event === 'created' || event.event === 'removed') {
		if (!route.query.i) {
			await refreshInstalledProjectIds()
			if (projectType.value === 'modpack') {
				if (event.event === 'removed') {
					syncHiddenInstanceProjectIds()
				}
				await searchState.refreshSearch()
			}
		}
	}

	if (instance.value && event.instance_id === instance.value.id && event.event === 'synced') {
		await refreshInstalledProjectIds()
		await searchState.refreshSearch()
	}
})

function getProjectBrowseQuery() {
	if (!browseRouteActive.value) {
		return undefined
	}
	if (!installContext.value) return undefined
	return {
		...route.query,
		b: route.fullPath,
	}
}

const advancedFiltersCollapsed = computed({
	get: () => appSettings.advancedFiltersCollapsed,
	set: (value) => {
		appSettings.advancedFiltersCollapsed = value
		getSettings()
			.then((settings) => {
				settings.advanced_filters_collapsed = value
				return setSettings(settings)
			})
			.catch(handleError)
	},
})

const dismissedPhotosensitivityFilterWarning = computed({
	get: () => appSettings.dismissedPhotosensitivityFilterWarning,
	set: (value) => {
		appSettings.dismissedPhotosensitivityFilterWarning = value
		getSettings()
			.then((settings) => {
				settings.dismissed_photosensitivity_filter_warning = value
				return setSettings(settings)
			})
			.catch(handleError)
	},
})

provideBrowseManager({
	tags,
	projectType,
	...searchState,
	advancedFiltersCollapsed,
	dismissedPhotosensitivityFilterWarning,
	getProjectLink: (result: Labrinth.Search.v3.ResultSearchProject) => ({
		path: `/project/${result.project_id ?? result.slug}`,
		query: getProjectBrowseQuery(),
	}),
	getServerProjectLink: (result: Labrinth.Search.v3.ResultSearchProject) => ({
		path: `/project/${result.slug ?? result.project_id}`,
		query: getProjectBrowseQuery(),
	}),
	selectableProjectTypes,
	showProjectTypeTabs: computed(() => !isServerContext.value),
	variant: 'app',
	getCardActions,
	installContext,
	providedFilters: combinedProvidedFilters,
	hideInstalled: computed({
		get: () => {
			if (projectType.value === 'modpack') return hideInstalledModpacks.value
			if (isServerContext.value) return serverHideInstalled.value
			return instanceHideInstalled.value
		},
		set: (val: boolean) => {
			if (projectType.value === 'modpack') {
				hideInstalledModpacks.value = val
				if (val) syncHiddenInstanceProjectIds()
				return
			}
			if (isServerContext.value) {
				serverHideInstalled.value = val
				if (val) syncHiddenServerContentProjectIds()
			} else {
				instanceHideInstalled.value = val
				if (val) syncHiddenInstanceProjectIds()
			}
		},
	}),
	showHideInstalled: computed(
		() =>
			projectType.value === 'modpack' ||
			// @ts-expect-error the line above already rules out modpack
			(isServerContext.value && projectType.value !== 'modpack') ||
			!!instance.value,
	),
	hideInstalledLabel: computed(() =>
		formatMessage(
			isFromWorlds.value
				? messages.hideAddedServers
				: projectType.value === 'modpack'
					? messages.hideInstalledModpacks
					: commonMessages.hideInstalledContentLabel,
		),
	),
	hideSelected: hideSelectedServerInstalls,
	showHideSelected: computed(
		() =>
			isServerContext.value &&
			projectType.value !== 'modpack' &&
			queuedServerInstallCount.value > 0,
	),
	hideSelectedLabel: computed(() => formatMessage(commonMessages.hideSelectedContentLabel)),
	onInstalled: onSearchResultInstalled,
	serverPings,
	getServerModpackContent,
	onContextMenu: handleRightClick,
	offline,
	lockedFilterMessages,
})
</script>

<template>
	<div class="flex flex-col gap-2 p-6">
		<BrowsePageLayout>
			<template #after>
				<ContextMenu ref="contextMenuRef" :label="formatMessage(messages.projectActionsLabel)">
					<template #open_link="{ option }">
						<GlobeIcon /> {{ option.label }} <ExternalIcon />
					</template>
				</ContextMenu>
			</template>
		</BrowsePageLayout>
		<CreationFlowModal
			v-if="isServerContext && projectType === 'modpack'"
			ref="serverSetupModalRef"
			:type="serverFlowFrom === 'reset-server' ? 'reset-server' : 'server-onboarding'"
			:available-loaders="['vanilla', 'fabric', 'neoforge', 'forge', 'quilt', 'paper', 'purpur']"
			:show-snapshot-toggle="true"
			:on-back="onServerFlowBack"
			:search-modpacks="searchServerModpacks"
			:get-project-versions="getServerProjectVersions"
			:get-loader-manifest="getLoaderManifest"
			@hide="() => {}"
			@browse-modpacks="() => {}"
			@create="handleServerModpackFlowCreate"
		/>
		<Teleport v-if="browseRouteActive" to="#sidebar-teleport-target">
			<BrowseSidebar />
		</Teleport>
	</div>
</template>
