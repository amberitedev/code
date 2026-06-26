<script setup lang="ts">
/**
 * Desktop browse route with instance and Core-server installation contexts.
 * - `initInstanceContext` (227) loads the active browse context and installed content.
 * - `selectableProjectTypes` (552) and `installContext` (598) derive the active UI state.
 * - `getCardActions` (787) creates app-specific install actions for each result.
 * - `search` (1090) normalizes results; `preloadProjectType` (1154) warms tab targets.
 * - `provideBrowseManager` (1334) exposes the layout context.
 */
import { installCoreModpack } from '@amberite/amberite-api'
import type { Labrinth } from '@modrinth/api-client'
import {
	CheckIcon,
	ClipboardCopyIcon,
	ExternalIcon,
	GlobeIcon,
	PlusIcon,
	SpinnerIcon,
} from '@modrinth/assets'
import type { BrowseInstallContentType, CardAction, ProjectType, Tags } from '@modrinth/ui'
import {
	BrowseSidebar,
	commonMessages,
	CreationFlowModal,
	defineMessages,
	getLatestMatchingInstallVersion,
	getSelectedInstallPreferences,
	getTargetInstallPreferences,
	injectNotificationManager,
	preferencesDiffer,
	provideBrowseManager,
	requestInstall,
	useBrowseSearch,
	useDebugLogger,
	useLoadingBarToken,
	useVIntl,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useStorage } from '@vueuse/core'
import type { Ref } from 'vue'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import type { LocationQuery } from 'vue-router'
import { onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'

import AppPageSkeleton from '@/components/ui/AppPageSkeleton.vue'
import AppBrowsePageLayout from '@/components/ui/AppBrowsePageLayout.vue'
import ContextMenu from '@/components/ui/ContextMenu.vue'
import { useAppServerBrowse } from '@/composables/browse/use-app-server-browse'
import { useCoreClient } from '@/composables/useCoreClient'
import {
	buildDiscoverSearchParams,
	DISCOVER_PRELOAD_STALE_MS,
	getDiscoverProjectTypeFromHref,
	type RawDiscoverSearchResults,
} from '@/composables/useDiscoverContentPreload'
import { useOptimisticLoading } from '@/composables/useOptimisticPreload'
import {
	get_project,
	get_project_v3,
	get_search_results_v3,
	get_version_many,
} from '@/helpers/cache.js'
import { profile_listener } from '@/helpers/events.js'
import { get_loader_versions as getLoaderManifest } from '@/helpers/metadata'
import { get_profile_from_pack_version } from '@/helpers/pack'
import {
	get as getInstance,
	get_installed_project_ids as getInstalledProjectIds,
} from '@/helpers/profile.js'
import { get_categories, get_game_versions, get_loaders } from '@/helpers/tags'
import { get_profile_worlds } from '@/helpers/worlds'
import { convertToSynced } from '@/pages/instance/synced/synced-conversion'
import { injectContentInstall } from '@/providers/content-install'
import { injectServerInstall } from '@/providers/server-install'
import {
	createServerInstallContent,
	provideServerInstallContent,
} from '@/providers/setup/server-install-content'
import { useBreadcrumbs } from '@/store/breadcrumbs'

defineOptions({
	name: 'BrowsePage',
})

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const { installingServerProjects, playServerProject, showAddServerToInstanceModal } =
	injectServerInstall()
const { install: installVersion } = injectContentInstall()
const queryClient = useQueryClient()
const debugLog = useDebugLogger('Browse')
const BROWSE_LOADING_BAR_STORAGE_KEY = 'app-browse-loading-bar-enabled'

const router = useRouter()
const route = useRoute()
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
	openServerModpackInstallFlow,
	onServerFlowBack,
	handleServerModpackFlowCreate,
	markServerProjectInstalled,
} = serverInstallContent

debugLog('fetching tags (categories, loaders, gameVersions)')
const categories = ref<Labrinth.Tags.v2.Category[]>([])
const loaders = ref<Labrinth.Tags.v2.Loader[]>([])
const availableGameVersions = ref<Labrinth.Tags.v2.GameVersion[]>([])
const tagsLoaded = ref(false)

const tags: Ref<Tags> = computed(() => ({
	gameVersions: availableGameVersions.value ?? [],
	loaders: loaders.value ?? [],
	categories: categories.value ?? [],
}))

type Instance = {
	game_version: string
	loader: string
	path: string
	install_stage: string
	icon_path?: string
	name: string
	linked_data?: {
		project_id: string
		version_id: string
		locked: boolean
	}
}

const instance: Ref<Instance | null> = ref(null)
const installedProjectIds: Ref<string[] | null> = ref(null)
const instanceHideInstalled = ref(false)
const newlyInstalled = ref<string[]>([])
const hiddenInstanceProjectIds = ref<Set<string>>(new Set())
const hiddenInstanceProjectIdsInitialized = ref(false)
const isServerInstance = ref(false)

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

const contextLoaded = ref(false)

async function refreshInstalledProjectIds() {
	if (!route.query.i) return

	if (route.query.from === 'worlds') {
		const worlds = await get_profile_worlds(route.query.i as string).catch(handleError)
		if (!worlds) return

		const serverProjectIds = worlds
			.filter((w) => w.type === 'server' && 'project_id' in w && w.project_id)
			.map((w) => (w as { project_id: string }).project_id)
		debugLog('installedServerProjectIds loaded', { count: serverProjectIds.length })
		installedProjectIds.value = serverProjectIds
		return
	}

	const ids = await getInstalledProjectIds(route.query.i as string).catch(handleError)
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
	await initServerContext()

	if (route.query.i) {
		instance.value = (await getInstance(route.query.i as string).catch(handleError)) ?? null
		debugLog('instance loaded', {
			name: instance.value?.name,
			loader: instance.value?.loader,
			gameVersion: instance.value?.game_version,
		})

		await refreshInstalledProjectIds()

		if (instance.value?.linked_data?.project_id) {
			debugLog('checking linked project for server status', instance.value.linked_data.project_id)
			const projectV3 = await get_project_v3(
				instance.value.linked_data.project_id,
				'must_revalidate',
			).catch(handleError)
			if (projectV3?.minecraft_server != null) {
				debugLog('instance is a server instance')
				isServerInstance.value = true
			}
		}
	}

	if (route.query.ai && !(route.params.projectType === 'modpack')) {
		debugLog('setting instanceHideInstalled from query', route.query.ai)
		instanceHideInstalled.value = route.query.ai === 'true'
	}
}

const instanceFilters = computed(() => {
	const filters = []

	if (instance.value) {
		const gameVersion = instance.value.game_version
		if (gameVersion) {
			filters.push({ type: 'game_version', option: gameVersion })
		}

		const platform = instance.value.loader
		const supportedModLoaders = ['fabric', 'forge', 'quilt', 'neoforge']

		if (platform && projectType.value === 'mod' && supportedModLoaders.includes(platform)) {
			filters.push({ type: 'mod_loader', option: platform })
		}

		if (isServerInstance.value) {
			filters.push({ type: 'environment', option: 'client' })
		}

		if (instanceHideInstalled.value && hiddenInstanceProjectIds.value.size > 0) {
			for (const id of hiddenInstanceProjectIds.value) {
				filters.push({ type: 'project_id', option: `project_id:${id}`, negative: true })
			}
		}
	}

	return filters
})

const serverHideInstalled = ref(false)
const hideSelectedServerInstalls = ref(false)
if (route.query.shi) {
	serverHideInstalled.value = route.query.shi === 'true'
}
const hiddenServerContentProjectIds = ref<Set<string>>(new Set())
const hiddenServerContentProjectIdsInitialized = ref(false)
const projectEnvironmentById = ref(new Map<string, Labrinth.Projects.v3.Environment[]>())

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

const serverContextFilters = computed(() => {
	const filters: { type: string; option: string; negative?: boolean }[] = []
	if (!serverContextServerData.value) return filters
	const pt = projectType.value

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
	}

	if (serverHideInstalled.value && hiddenServerContentProjectIds.value.size > 0) {
		for (const id of hiddenServerContentProjectIds.value) {
			filters.push({ type: 'project_id', option: `project_id:${id}`, negative: true })
		}
	}

	return filters
})

const combinedProvidedFilters = computed(() =>
	isServerContext.value ? serverContextFilters.value : instanceFilters.value,
)

const {
	serverPings,
	contextMenuRef,
	updateServerHits,
	getServerModpackContent,
	getServerCardActions,
	handleRightClick,
	handleOptionsClick,
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
window.addEventListener('offline', () => {
	debugLog('went offline')
	offline.value = true
})
window.addEventListener('online', () => {
	debugLog('went online')
	offline.value = false
})

const messages = defineMessages({
	addServersToInstance: {
		id: 'app.browse.add-servers-to-instance',
		defaultMessage: 'Adding server to instance',
	},
	addToAnInstance: {
		id: 'app.browse.add-to-an-instance',
		defaultMessage: 'Add to an instance',
	},
	discoverContent: {
		id: 'app.browse.discover-content',
		defaultMessage: 'Discover content',
	},
	discoverServers: {
		id: 'app.browse.discover-servers',
		defaultMessage: 'Discover servers',
	},
	environmentProvidedByServer: {
		id: 'search.filter.locked.server-environment.title',
		defaultMessage: 'Only client-side mods can be added to the server instance',
	},
	gameVersionProvidedByInstance: {
		id: 'search.filter.locked.instance-game-version.title',
		defaultMessage: 'Game version is provided by the instance',
	},
	gameVersionProvidedByServer: {
		id: 'search.filter.locked.server-game-version.title',
		defaultMessage: 'Game version is provided by the server',
	},
	hideAddedServers: {
		id: 'app.browse.hide-added-servers',
		defaultMessage: 'Hide already added servers',
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
			'Adding content can break compatibility when joining the server. Any added content will also be lost when you update the server instance content.',
	},
	modLoaderProvidedByInstance: {
		id: 'search.filter.locked.instance-loader.title',
		defaultMessage: 'Loader is provided by the instance',
	},
	modpacksProjectType: {
		id: 'app.browse.project-type.modpacks',
		defaultMessage: 'Modpacks',
	},
	modLoaderProvidedByServer: {
		id: 'search.filter.locked.server-loader.title',
		defaultMessage: 'Loader is provided by the server',
	},
	providedByInstance: {
		id: 'search.filter.locked.instance',
		defaultMessage: 'Provided by the instance',
	},
	providedByServer: {
		id: 'search.filter.locked.server',
		defaultMessage: 'Provided by the server',
	},
	syncFilterButton: {
		id: 'search.filter.locked.instance.sync',
		defaultMessage: 'Sync with instance',
	},
	installOnServer: {
		id: 'app.browse.card-action.install-on-server',
		defaultMessage: 'Install on server',
	},
	installSynced: {
		id: 'app.browse.card-action.install-synced',
		defaultMessage: 'Install synced',
	},
})

const breadcrumbs = useBreadcrumbs()
const browseTitle = computed(() =>
	formatMessage(isFromWorlds.value ? messages.discoverServers : messages.discoverContent),
)
breadcrumbs.setName('BrowseTitle', browseTitle.value)
if (instance.value) {
	const instanceLink = `/instance/${encodeURIComponent(instance.value.path)}`
	breadcrumbs.setContext({
		name: instance.value.name,
		link: isFromWorlds.value ? `${instanceLink}/worlds` : instanceLink,
	})
} else {
	breadcrumbs.setContext(null)
}

onBeforeRouteLeave(() => {
	breadcrumbs.setContext({
		name: browseTitle.value,
		link: `/browse/${projectType.value}`,
		query: route.query,
	})
})

const projectType = ref<ProjectType>(route.params.projectType as ProjectType)

watch(
	() => route.params.projectType as ProjectType,
	async (newType) => {
		if (isSetupServerContext.value) {
			enforceSetupModpackRoute(newType)
			if (newType !== 'modpack') return
		}

		if (!newType || newType === projectType.value) return

		debugLog('projectType route param changed', { from: projectType.value, to: newType })
		projectType.value = newType

		if (!route.query.i && instance.value) {
			debugLog('instance context removed, resetting')
			instance.value = null
			installedProjectIds.value = null
			instanceHideInstalled.value = false
			newlyInstalled.value = []
			isServerInstance.value = false
			breadcrumbs.setName('BrowseTitle', formatMessage(messages.discoverContent))
			breadcrumbs.setContext(null)
		}
	},
)

function createBrowseTab(
	type: ProjectType,
	label: string,
	suffix: string,
	shown: boolean | undefined = undefined,
) {
	return {
		label,
		href: `/browse/${type}${suffix}`,
		shown,
		onHover: () => preloadProjectType(type),
	}
}

const browseTabSuffix = computed(() => {
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

	const suffix = browseTabSuffix.value

	if (isSetupServerContext.value) {
		return [
			createBrowseTab('modpack', formatMessage(messages.modpacksProjectType), suffix),
		]
	}

	if (isFromWorlds.value) {
		return [createBrowseTab('server', 'Servers', suffix)]
	}

	return [
		createBrowseTab('modpack', 'Modpacks', suffix, modpacks),
		createBrowseTab('mod', 'Mods', suffix, mods),
		createBrowseTab('resourcepack', 'Resource Packs', suffix),
		createBrowseTab('datapack', 'Data Packs', suffix, dataPacks),
		createBrowseTab('shader', 'Shaders', suffix),
		createBrowseTab('server', 'Servers', suffix, !instance.value),
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
			iconSrc: instance.value.icon_path ? convertFileSrc(instance.value.icon_path) : null,
			backUrl: `/instance/${encodeURIComponent(instance.value.path)}${isFromWorlds.value ? '/worlds' : ''}`,
			backLabel: formatMessage(messages.backToInstance),
			heading: formatMessage(
				isFromWorlds.value ? messages.addServersToInstance : commonMessages.installingContentLabel,
			),
			warning:
				isServerInstance.value && !isFromWorlds.value
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
	project: Labrinth.Search.v2.ResultSearchProject & Labrinth.Search.v3.ResultSearchProject,
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
		projectTypeValue,
	)

	if (!selectedVersion) {
		return { versionId: null as string | null }
	}

	return { versionId: selectedVersion.id }
}

async function getDefaultModpackVersionId(projectId: string) {
	const project = await get_project(projectId, 'must_revalidate')
	const versionId = project.versions[project.versions.length - 1]
	if (!versionId) throw new Error('No installable modpack version found.')
	return versionId
}

async function createCoreServerWithModpack(args: {
	projectId: string
	versionId: string
	title: string
	iconUrl?: string
}) {
	const core = useCoreClient()
	const packProfile = await get_profile_from_pack_version(
		args.projectId,
		args.versionId,
		args.title,
		args.iconUrl,
	)
	const { instance } = await installCoreModpack(core, {
		projectId: args.projectId,
		versionId: args.versionId,
		profile: packProfile,
	})
	return instance.id
}

async function getProjectEnvironment(projectId: string) {
	const cached = projectEnvironmentById.value.get(projectId)
	if (cached) return cached

	const project = await get_project_v3(projectId, 'must_revalidate').catch(handleError)
	const environment = project?.environment ?? []
	projectEnvironmentById.value = new Map(projectEnvironmentById.value).set(projectId, environment)
	return environment
}

function supportsDedicatedServerEnvironment(project: {
	environment?: Labrinth.Projects.v3.Environment[]
	server_side?: string
}) {
	if (project.environment?.length) {
		return project.environment.some((environment) =>
			[
				'server_only',
				'dedicated_server_only',
				'client_and_server',
				'client_only_server_optional',
				'server_only_client_optional',
				'client_or_server',
				'client_or_server_prefers_both',
			].includes(environment),
		)
	}

	return project.server_side !== 'unsupported'
}

type AppCardAction = CardAction & {
	joinedActions?: AppCardAction[]
}

function getCardActions(
	result: Labrinth.Search.v2.ResultSearchProject | Labrinth.Search.v3.ResultSearchProject,
	currentProjectType: string,
): AppCardAction[] {
	if (currentProjectType === 'server') {
		return getServerCardActions(result as Labrinth.Search.v3.ResultSearchProject)
	}

	// Non-server project actions
	const projectResult = result as (Labrinth.Search.v2.ResultSearchProject &
		Labrinth.Search.v3.ResultSearchProject) & {
		environment?: Labrinth.Projects.v3.Environment[]
		installed?: boolean
		installing?: boolean
	}
	const isInstalled =
		projectResult.installed ||
		allInstalledIds.value.has(projectResult.project_id || '') ||
		serverContentProjectIds.value.has(projectResult.project_id || '') ||
		serverContextServerData.value?.upstream?.project_id === projectResult.project_id
	const isInstalling = installingProjectIds.value.has(projectResult.project_id)
	const projectTitle = projectResult.title ?? projectResult.name ?? 'Modpack'
	const projectIconUrl = projectResult.icon_url ?? undefined
	const isServerInstallSupported = supportsDedicatedServerEnvironment(projectResult)

	async function installModpackOnClient() {
		const selectedPreferences = getCurrentSelectedInstallPreferences(currentProjectType)
		return await new Promise<{ profilePath: string; versionId: string }>((resolve, reject) => {
			let profilePath: string | null = null
			let versionId: string | null = null

			function finish() {
				if (profilePath && versionId) {
					resolve({ profilePath, versionId })
				}
			}

			void installVersion(
				projectResult.project_id,
				null,
				null,
				'SearchCard',
				(installedVersionId) => {
					if (!installedVersionId) {
						reject(new Error('Modpack install was cancelled.'))
						return
					}
					versionId = installedVersionId
					onSearchResultInstalled(projectResult.project_id)
					finish()
				},
				(createdProfilePath) => {
					profilePath = createdProfilePath
					finish()
				},
				{
					preferredLoader: selectedPreferences.loaders?.[0],
					preferredGameVersion: selectedPreferences.gameVersions?.[0],
				},
			).catch(reject)
		})
	}

	async function installOnClient() {
		setProjectInstalling(projectResult.project_id, true)
		try {
			const { profilePath } = await installModpackOnClient()
			await router.push(`/instance/${encodeURIComponent(profilePath)}`)
		} catch (err) {
			handleError(err)
		} finally {
			setProjectInstalling(projectResult.project_id, false)
		}
	}

	async function installSynced() {
		setProjectInstalling(projectResult.project_id, true)
		try {
			const { profilePath, versionId } = await installModpackOnClient()
			const coreInstanceId = await convertToSynced(profilePath)
			await useCoreClient().installModpackVersion(
				coreInstanceId,
				projectResult.project_id,
				versionId,
			)
			await router.push(`/instance/${encodeURIComponent(profilePath)}`)
		} catch (err) {
			handleError(err)
		} finally {
			setProjectInstalling(projectResult.project_id, false)
		}
	}

	async function installOnServer() {
		setProjectInstalling(projectResult.project_id, true)
		try {
			const versionId = await getDefaultModpackVersionId(projectResult.project_id)
			const coreInstanceId = await createCoreServerWithModpack({
				projectId: projectResult.project_id,
				versionId,
				title: projectTitle,
				iconUrl: projectIconUrl,
			})
			await router.push(`/instance/${encodeURIComponent(coreInstanceId)}`)
		} catch (err) {
			handleError(err)
		} finally {
			setProjectInstalling(projectResult.project_id, false)
		}
	}

	function getModpackInstallActions() {
		if (currentProjectType !== 'modpack') return undefined
		if (!isServerInstallSupported) return undefined

		return [
			{
				key: 'install-synced',
				label: formatMessage(messages.installSynced),
				icon: PlusIcon,
				onClick: installSynced,
			},
			{
				key: 'install-on-server',
				label: formatMessage(messages.installOnServer),
				icon: PlusIcon,
				onClick: installOnServer,
			},
		]
	}

	if (
		isServerContext.value &&
		['modpack', 'mod', 'plugin', 'datapack'].includes(currentProjectType)
	) {
		const isQueued = queuedServerInstallProjectIds.value.has(projectResult.project_id)
		const isInstallingSelection = isInstallingQueuedServerInstalls.value
		const validatingInstall =
			isInstalling && currentProjectType !== 'modpack' && !isInstallingSelection
		async function installServerContextContent() {
			if (isQueued) {
				removeQueuedServerInstall(projectResult.project_id)
				return
			}

			const contentType = currentProjectType as BrowseInstallContentType
			const isModpack = contentType === 'modpack'
			const shouldShowInstalling = isModpack || !isQueued
			if (shouldShowInstalling) {
				setProjectInstalling(projectResult.project_id, true)
			}
			try {
				await requestInstall({
					project: projectResult,
					contentType,
					mode: isModpack ? 'immediate' : 'queue',
					selectedFilters: isModpack ? [] : searchState.currentFilters.value,
					providedFilters: isModpack ? [] : combinedProvidedFilters.value,
					overriddenProvidedFilterTypes: isModpack
						? []
						: searchState.overriddenProvidedFilterTypes.value,
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
			} catch (err) {
				handleError(err as Error)
			} finally {
				if (shouldShowInstalling) {
					setProjectInstalling(projectResult.project_id, false)
				}
			}
		}
		const modpackInstallActions = getModpackInstallActions()
		const installLabel = isInstalled
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
						: isQueued || isInstalled
							? CheckIcon
							: PlusIcon,
				iconClass: isInstalling || isInstallingSelection ? 'animate-spin' : undefined,
				disabled: isInstalled || isInstalling || isInstallingSelection,
				color: isQueued && !isInstalling && !isInstallingSelection ? 'green' : 'brand',
				type: 'standard',
				joinedActions:
					modpackInstallActions && !isInstalled && !isInstalling && !isInstallingSelection
						? modpackInstallActions
						: undefined,
				onClick: currentProjectType === 'modpack' ? installOnClient : installServerContextContent,
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
					: isInstalled
						? commonMessages.installedLabel
						: shouldUseInstallIcon
							? commonMessages.installButton
							: messages.addToAnInstance,
			),
			icon: isInstalling ? SpinnerIcon : isInstalled ? CheckIcon : PlusIcon,
			iconClass: isInstalling ? 'animate-spin' : undefined,
			disabled: isInstalled || isInstalling,
			color: 'brand',
			type: 'standard',
			joinedActions:
				isModpack && !isInstalled && !isInstalling ? getModpackInstallActions() : undefined,
			onClick: isModpack
				? installOnClient
				: async () => {
						setProjectInstalling(projectResult.project_id, true)
						try {
							const selectedInstall = instance.value
								? await chooseInstanceInstallVersion(projectResult, currentProjectType)
								: { versionId: null as string | null }
							if (selectedInstall === null) {
								setProjectInstalling(projectResult.project_id, false)
								return
							}
							const selectedPreferences = getCurrentSelectedInstallPreferences(currentProjectType)
							await installVersion(
								projectResult.project_id,
								selectedInstall.versionId,
								instance.value ? instance.value.path : null,
								'SearchCard',
								(versionId, installedProjectIds) => {
									setProjectInstalling(projectResult.project_id, false)
									if (versionId) {
										onSearchResultsInstalled(
											installedProjectIds ?? [projectResult.project_id],
										)
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

async function search(requestParams: string, searchProjectType: ProjectType = projectType.value) {
	debugLog('searching v3', requestParams)
	const isServer = searchProjectType === 'server'

	const rawResults = await queryClient.fetchQuery({
		queryKey: ['search', 'v3', requestParams],
		queryFn: () => get_search_results_v3(requestParams) as Promise<RawDiscoverSearchResults>,
		staleTime: DISCOVER_PRELOAD_STALE_MS,
	})

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

	const hits = await Promise.all(
		rawResults.result.hits.map(async (hit) => {
			const mapped = {
				...hit,
				title: hit.name,
				description: hit.summary,
			} as unknown as Labrinth.Search.v2.ResultSearchProject & {
				environment?: Labrinth.Projects.v3.Environment[]
				installed?: boolean
			}

			if (searchProjectType === 'modpack' || hit.project_types?.includes('modpack')) {
				mapped.environment = await getProjectEnvironment(hit.project_id)
			}

			if (instance.value || isServerContext.value) {
				const installedIds = instance.value
					? new Set([...newlyInstalled.value, ...(installedProjectIds.value ?? [])])
					: serverContentProjectIds.value
				mapped.installed = installedIds.has(hit.project_id)
			}

			return mapped
		}),
	)

	return {
		projectHits: hits,
		serverHits: [],
		total_hits: rawResults.result.total_hits,
		per_page: rawResults.result.hits_per_page,
	}
}

function preloadProjectType(type: ProjectType) {
	if (!tagsLoaded.value || !contextLoaded.value || type === projectType.value) return

	const requestParams = buildDiscoverSearchParams(type)
	void search(requestParams, type).catch(() => undefined)
}

function preloadSelectableProjectTypes() {
	for (const link of selectableProjectTypes.value) {
		if (link.shown === false) continue

		const type = getDiscoverProjectTypeFromHref(link.href)
		if (type) preloadProjectType(type)
	}
}

const isServerFilterContext = computed(() => isServerContext.value || isServerInstance.value)

const lockedFilterMessages = computed(() => ({
	gameVersion: formatMessage(
		isServerFilterContext.value
			? messages.gameVersionProvidedByServer
			: messages.gameVersionProvidedByInstance,
	),
	modLoader: formatMessage(
		isServerFilterContext.value
			? messages.modLoaderProvidedByServer
			: messages.modLoaderProvidedByInstance,
	),
	environment: formatMessage(messages.environmentProvidedByServer),
	syncButton: formatMessage(messages.syncFilterButton),
	providedBy: formatMessage(
		isServerFilterContext.value ? messages.providedByServer : messages.providedByInstance,
	),
}))

const searchState = useBrowseSearch({
	projectType,
	tags,
	providedFilters: combinedProvidedFilters,
	search,
	immediateProjectTypeSearch: true,
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
		() => searchState.currentFilters.value,
		() => searchState.serverCurrentFilters.value,
		() => projectType.value,
	],
	() => {
		if (isServerContext.value) {
			syncHiddenServerContentProjectIds()
		} else if (instance.value) {
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

if (instance.value?.game_version) {
	const gv = instance.value.game_version
	const alreadyHasGv = searchState.serverCurrentFilters.value.some(
		(f) => f.type === 'server_game_version' && f.option === gv,
	)
	if (!alreadyHasGv) {
		searchState.serverCurrentFilters.value.push({ type: 'server_game_version', option: gv })
	}
}

const browseInitialPending = computed(
	() =>
		!tagsLoaded.value ||
		!contextLoaded.value ||
		(searchState.loading.value &&
			(searchState.isServerType.value
				? searchState.serverHits.value.length === 0
				: searchState.projectHits.value.length === 0)),
)
const hasBrowseContent = computed(() =>
	searchState.isServerType.value
		? searchState.serverHits.value.length > 0
		: searchState.projectHits.value.length > 0,
)
const browseLoadingBarEnabled = useStorage(BROWSE_LOADING_BAR_STORAGE_KEY, false)
const browseLoadingBarPending = computed(
	() => browseLoadingBarEnabled.value && browseInitialPending.value,
)
const showBrowseSkeleton = useOptimisticLoading(browseInitialPending, hasBrowseContent)
useLoadingBarToken(browseLoadingBarPending)

onMounted(async () => {
	contextLoaded.value = false
	const [nextCategories, nextLoaders, nextGameVersions] = await Promise.all([
		queryClient
			.fetchQuery({
				queryKey: ['tags', 'categories'],
				queryFn: get_categories,
				staleTime: 10 * 60_000,
			})
			.catch(handleError),
		queryClient
			.fetchQuery({
				queryKey: ['tags', 'loaders'],
				queryFn: get_loaders,
				staleTime: 10 * 60_000,
			})
			.catch(handleError),
		queryClient
			.fetchQuery({
				queryKey: ['tags', 'game-versions'],
				queryFn: get_game_versions,
				staleTime: 10 * 60_000,
			})
			.catch(handleError),
	])
	categories.value = nextCategories ?? []
	loaders.value = nextLoaders ?? []
	availableGameVersions.value = nextGameVersions ?? []
	tagsLoaded.value = true
	await initInstanceContext()
	contextLoaded.value = true
	await searchState.refreshSearch()
	preloadSelectableProjectTypes()
})

type UnlistenFn = () => void

let isUnmounted = false
let unlistenProfiles: UnlistenFn | null = null

onMounted(() => {
	profile_listener(async (event: { event: string; profile_path_id: string }) => {
		if (
			instance.value &&
			event.profile_path_id === instance.value.path &&
			event.event === 'synced'
		) {
			await refreshInstalledProjectIds()
			await searchState.refreshSearch()
		}
	})
		.then((unlisten) => {
			if (isUnmounted) {
				unlisten()
				return
			}

			unlistenProfiles = unlisten
		})
		.catch(handleError)
})

onUnmounted(() => {
	isUnmounted = true
	unlistenProfiles?.()
})

function getProjectBrowseQuery() {
	if (!installContext.value) return undefined
	return {
		...route.query,
		b: route.fullPath,
	}
}

provideBrowseManager({
	tags,
	projectType,
	...searchState,
	getProjectLink: (result: Labrinth.Search.v2.ResultSearchProject) => ({
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
		get: () => (isServerContext.value ? serverHideInstalled.value : instanceHideInstalled.value),
		set: (val: boolean) => {
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
		() => (isServerContext.value && projectType.value !== 'modpack') || !!instance.value,
	),
	hideInstalledLabel: computed(() =>
		formatMessage(
			isFromWorlds.value ? messages.hideAddedServers : commonMessages.hideInstalledContentLabel,
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
	<div class="flex flex-col gap-3 p-6">
		<AppPageSkeleton v-if="showBrowseSkeleton" variant="browse" class="!p-0" />
		<AppBrowsePageLayout v-else-if="!browseInitialPending || hasBrowseContent">
			<template #after>
				<ContextMenu ref="contextMenuRef" @option-clicked="handleOptionsClick">
					<template #open_link>
						<GlobeIcon /> {{ formatMessage(commonMessages.openInModrinthButton) }} <ExternalIcon />
					</template>
					<template #copy_link>
						<ClipboardCopyIcon /> {{ formatMessage(commonMessages.copyLinkButton) }}
					</template>
				</ContextMenu>
			</template>
		</AppBrowsePageLayout>
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
		<Teleport to="#sidebar-teleport-target">
			<BrowseSidebar />
		</Teleport>
	</div>
</template>
