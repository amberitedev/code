<script setup lang="ts">
/**
 * Desktop browse route with instance and Core-server installation contexts.
 * - `initInstanceContext` (232) loads the active browse context and installed content.
 * - `selectableProjectTypes` (557) and `installContext` (601) derive the active UI state.
 * - `getCardActions` (870) creates app-specific install actions for each result.
 * - `search` (1171) normalizes results; `preloadProjectType` (1236) warms tab targets.
 * - `provideBrowseManager` (1430) exposes the layout context.
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
import type {
	BrowseInstallContentType,
	CardAction,
	ProjectType,
	Tags,
	UiMotionDirection,
} from '@modrinth/ui'
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
	useVIntl,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { convertFileSrc } from '@tauri-apps/api/core'
import type { Ref } from 'vue'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import type { LocationQuery } from 'vue-router'
import { onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'

import AppBrowsePageGhost from '@/components/ui/AppBrowsePageGhost.vue'
import AppBrowsePageLayout from '@/components/ui/AppBrowsePageLayout.vue'
import ContextMenu from '@/components/ui/ContextMenu.vue'
import { useAppServerBrowse } from '@/composables/browse/use-app-server-browse'
import { useCoreClient } from '@/composables/useCoreClient'
import {
	buildDiscoverSearchParams,
	DISCOVER_METADATA_STALE_MS,
	DISCOVER_PRELOAD_STALE_MS,
	getDiscoverProjectTypeFromHref,
	type RawDiscoverSearchResults,
} from '@/composables/useDiscoverContentPreload'
import {
	get_project,
	get_project_v3,
	get_search_results_v3,
	get_version_many,
} from '@/helpers/cache.js'
import { instance_listener } from '@/helpers/events.js'
import {
	get as getInstance,
	get_installed_project_ids as getInstalledProjectIds,
} from '@/helpers/instance'
import { get_loader_versions as getLoaderManifest } from '@/helpers/metadata'
import { get_profile_from_pack_version } from '@/helpers/pack'
import { get as getSettings, set as setSettings } from '@/helpers/settings.ts'
import { get_categories, get_game_versions, get_loaders } from '@/helpers/tags'
import { convertToSynced } from '@/pages/instance/synced/synced-conversion'
import { injectContentInstall } from '@/providers/content-install'
import { injectServerInstall } from '@/providers/server-install'
import {
	createServerInstallContent,
	provideServerInstallContent,
} from '@/providers/setup/server-install-content'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { useTheming } from '@/store/state'

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

const router = useRouter()
const route = useRoute()
const themeStore = useTheming()
const browseRouteActive = computed(() => route.path.startsWith('/browse/'))
const serverSetupModalRef = ref<InstanceType<typeof CreationFlowModal> | null>(null)
const browseTransitioning = ref(false)
const browseTransitionDirection = ref<UiMotionDirection>('forward')
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
const TAG_CATEGORIES_QUERY_KEY = ['tags', 'categories'] as const
const TAG_LOADERS_QUERY_KEY = ['tags', 'loaders'] as const
const TAG_GAME_VERSIONS_QUERY_KEY = ['tags', 'game-versions'] as const

function getFreshQueryData<T>(queryKey: readonly unknown[], staleTime: number) {
	const state = queryClient.getQueryState<T>(queryKey)
	if (!state || state.data === undefined) return undefined
	if (Date.now() - state.dataUpdatedAt > staleTime) return undefined
	return state.data
}

const cachedCategories = getFreshQueryData<Labrinth.Tags.v2.Category[]>(
	TAG_CATEGORIES_QUERY_KEY,
	DISCOVER_METADATA_STALE_MS,
)
const cachedLoaders = getFreshQueryData<Labrinth.Tags.v2.Loader[]>(
	TAG_LOADERS_QUERY_KEY,
	DISCOVER_METADATA_STALE_MS,
)
const cachedGameVersions = getFreshQueryData<Labrinth.Tags.v2.GameVersion[]>(
	TAG_GAME_VERSIONS_QUERY_KEY,
	DISCOVER_METADATA_STALE_MS,
)

const categories = ref<Labrinth.Tags.v2.Category[]>(cachedCategories ?? [])
const loaders = ref<Labrinth.Tags.v2.Loader[]>(cachedLoaders ?? [])
const availableGameVersions = ref<Labrinth.Tags.v2.GameVersion[]>(cachedGameVersions ?? [])
const tagsLoaded = ref(!!cachedCategories && !!cachedLoaders && !!cachedGameVersions)

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
	profile_type?: 'client' | 'server' | 'synced'
	icon_path?: string
	name: string
	link?: {
		type: string
		project_id: string
		version_id: string
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

function hasAsyncBrowseContext() {
	return !!route.query.i || !!serverIdQuery.value
}

const contextLoaded = ref(!hasAsyncBrowseContext())

async function refreshInstalledProjectIds() {
	if (!route.query.i) return

	if (route.query.from === 'worlds') {
		const worlds = await get_instance_worlds(route.query.i as string).catch(handleError)
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

		if (instance.value?.link?.project_id) {
			debugLog('checking linked project for server status', instance.value.link.project_id)
			const projectV3 = await get_project_v3(
				instance.value.link.project_id,
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
		defaultMessage: 'Hide servers already added',
	},
	installingToServer: {
		id: 'app.browse.server.installing',
		defaultMessage: 'Installing',
	},
	backToInstance: {
		id: 'app.browse.back-to-instance',
		defaultMessage: 'Back to instance',
	},
	clientProfileType: {
		id: 'app.browse.profile-type.client',
		defaultMessage: 'Client',
	},
	serverProfileType: {
		id: 'app.browse.profile-type.server',
		defaultMessage: 'Server',
	},
	syncedProfileType: {
		id: 'app.browse.profile-type.synced',
		defaultMessage: 'Synced',
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
	const instanceLink = `/instance/${encodeURIComponent(instance.value.id)}`
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

function resetInstanceContext() {
	if (!instance.value) return

	debugLog('instance context removed, resetting')
	instance.value = null
	installedProjectIds.value = null
	instanceHideInstalled.value = false
	newlyInstalled.value = []
	hiddenInstanceProjectIds.value = new Set()
	hiddenInstanceProjectIdsInitialized.value = false
	isServerInstance.value = false
	breadcrumbs.setName('BrowseTitle', formatMessage(messages.discoverContent))
	breadcrumbs.setContext(null)
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
	(instanceId) => {
		if (!instanceId && route.path.startsWith('/browse')) {
			resetInstanceContext()
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
		return [createBrowseTab('modpack', formatMessage(messages.modpacksProjectType), suffix)]
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

function getProfileTypeLabel(profileType: Instance['profile_type']) {
	if (profileType === 'server') return formatMessage(messages.serverProfileType)
	if (profileType === 'synced') return formatMessage(messages.syncedProfileType)
	return formatMessage(messages.clientProfileType)
}

const installContext = computed(() => {
	if (isServerContext.value && serverContextServerData.value) {
		return {
			name: serverContextServerData.value.name,
			loader: serverContextServerData.value.loader ?? '',
			gameVersion: serverContextServerData.value.mc_version ?? '',
			profileTypeLabel: formatMessage(messages.serverProfileType),
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
			skipNonEssentialWarnings: themeStore.getFeatureFlag('skip_non_essential_warnings'),
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
			profileTypeLabel: getProfileTypeLabel(instance.value.profile_type),
			iconSrc: instance.value.icon_path ? convertFileSrc(instance.value.icon_path) : null,
			backUrl: `/instance/${encodeURIComponent(instance.value.id)}${isFromWorlds.value ? '/worlds' : ''}`,
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

function runAfterBrowseFrame(callback: () => void) {
	requestAnimationFrame(() => {
		const windowWithIdleCallback = window as Window & {
			requestIdleCallback?: (callback: () => void, options?: { timeout?: number }) => number
		}

		if (windowWithIdleCallback.requestIdleCallback) {
			windowWithIdleCallback.requestIdleCallback(() => callback(), { timeout: 750 })
			return
		}

		setTimeout(callback, 0)
	})
}

type BrowseProjectHit = Labrinth.Search.v2.ResultSearchProject &
	Labrinth.Search.v3.ResultSearchProject & {
		environment?: Labrinth.Projects.v3.Environment[]
		installed?: boolean
	}

function shouldHydrateProjectEnvironment(
	project: BrowseProjectHit,
	searchProjectType: ProjectType,
) {
	return (
		(searchProjectType === 'modpack' || project.project_types?.includes('modpack')) &&
		!projectEnvironmentById.value.has(project.project_id)
	)
}

function scheduleProjectEnvironmentHydration(
	hits: BrowseProjectHit[],
	searchProjectType: ProjectType,
	requestParams: string,
) {
	const projectIds = hits
		.filter((hit) => shouldHydrateProjectEnvironment(hit, searchProjectType))
		.map((hit) => hit.project_id)

	if (projectIds.length === 0) return

	runAfterBrowseFrame(() => {
		void hydrateProjectEnvironments(projectIds, searchProjectType, requestParams)
	})
}

async function hydrateProjectEnvironments(
	projectIds: string[],
	searchProjectType: ProjectType,
	requestParams: string,
) {
	const updates = new Map<string, Labrinth.Projects.v3.Environment[]>()

	await Promise.all(
		projectIds.map(async (projectId) => {
			const environment = await getProjectEnvironment(projectId)
			updates.set(projectId, environment)
		}),
	)

	if (updates.size === 0) return
	if (searchState.activeResultKey.value !== `${searchProjectType}:${requestParams}`) return

	let changed = false
	const nextHits = searchState.projectHits.value.map((hit) => {
		const environment = updates.get(hit.project_id)
		if (environment === undefined || hit.environment === environment) {
			return hit
		}

		changed = true
		return { ...hit, environment }
	})

	if (changed) {
		searchState.projectHits.value = nextHits
	}
}

type AppCardAction = CardAction & {
	joinedActions?: AppCardAction[]
}

function getCardActions(
	result: Labrinth.Search.v3.ResultSearchProject,
	currentProjectType: string,
): AppCardAction[] {
	if (currentProjectType === 'server') {
		return getServerCardActions(result)
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

function createBrowseSearchResponse(
	rawResults: RawDiscoverSearchResults,
	searchProjectType: ProjectType,
	requestParams: string,
) {
	const isServer = searchProjectType === 'server'

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

	const hits = rawResults.result.hits.map((hit) => {
		const mapped: Labrinth.Search.v3.ResultSearchProject & { installed?: boolean } = {
			...hit,
			title: hit.name,
			description: hit.summary,
		} as unknown as BrowseProjectHit

		const cachedEnvironment = projectEnvironmentById.value.get(hit.project_id)
		if (
			cachedEnvironment &&
			(searchProjectType === 'modpack' || hit.project_types?.includes('modpack'))
		) {
			mapped.environment = cachedEnvironment
		}

		if (instance.value || isServerContext.value) {
			const installedIds = instance.value
				? new Set([...newlyInstalled.value, ...(installedProjectIds.value ?? [])])
				: serverContentProjectIds.value
			mapped.installed = installedIds.has(hit.project_id)
		}

		return mapped
	})

	scheduleProjectEnvironmentHydration(hits, searchProjectType, requestParams)

	return {
		projectHits: hits,
		serverHits: [],
		total_hits: rawResults.result.total_hits,
		per_page: rawResults.result.hits_per_page,
	}
}

function getCachedSearchResponse(requestParams: string, searchProjectType: string) {
	if (!tagsLoaded.value) return undefined
	if (hasAsyncBrowseContext()) return undefined

	const rawResults = getFreshQueryData<RawDiscoverSearchResults>(
		['search', 'v3', requestParams],
		DISCOVER_PRELOAD_STALE_MS,
	)
	if (rawResults === undefined) return undefined

	return createBrowseSearchResponse(rawResults, searchProjectType as ProjectType, requestParams)
}

async function search(requestParams: string, searchProjectType: ProjectType = projectType.value) {
	debugLog('searching v3', requestParams)
	const rawResults = await queryClient.fetchQuery({
		queryKey: ['search', 'v3', requestParams],
		queryFn: () => get_search_results_v3(requestParams) as Promise<RawDiscoverSearchResults>,
		staleTime: DISCOVER_PRELOAD_STALE_MS,
	})

	return createBrowseSearchResponse(rawResults, searchProjectType, requestParams)
}

function preloadProjectType(type: ProjectType) {
	if (!tagsLoaded.value || !contextLoaded.value || type === projectType.value) return

	const requestParams = buildDiscoverSearchParams(type, searchState.maxResults.value)
	void queryClient
		.prefetchQuery({
			queryKey: ['search', 'v3', requestParams],
			queryFn: () => get_search_results_v3(requestParams) as Promise<RawDiscoverSearchResults>,
			staleTime: DISCOVER_PRELOAD_STALE_MS,
		})
		.catch(() => undefined)
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
	active: browseRouteActive,
	providedFilters: combinedProvidedFilters,
	search,
	getCachedSearchResponse,
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

const BROWSE_GHOST_DELAY_MS = 80
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
const hasMountedBrowseLayout = ref(false)
const browseReadyForLayout = computed(() => !browseInitialPending.value || hasBrowseContent.value)
watch(
	browseReadyForLayout,
	(ready) => {
		if (ready) {
			hasMountedBrowseLayout.value = true
		}
	},
	{ immediate: true },
)
const browseGhostVisible = ref(false)
let browseGhostDelayTimer: ReturnType<typeof setTimeout> | null = null
const showBrowseGhost = computed(
	() => browseGhostVisible.value && !hasMountedBrowseLayout.value && browseInitialPending.value,
)
const showBrowseLayout = computed(() => hasMountedBrowseLayout.value)

function clearBrowseGhostDelayTimer() {
	if (browseGhostDelayTimer === null) return

	clearTimeout(browseGhostDelayTimer)
	browseGhostDelayTimer = null
}

function scheduleBrowseGhostDelay() {
	clearBrowseGhostDelayTimer()
	browseGhostDelayTimer = setTimeout(() => {
		browseGhostDelayTimer = null
		if (browseInitialPending.value && !hasMountedBrowseLayout.value) {
			browseGhostVisible.value = true
		}
	}, BROWSE_GHOST_DELAY_MS)
}

watch(
	() => [browseInitialPending.value, hasMountedBrowseLayout.value],
	([pending, mounted]) => {
		if (pending && !mounted) {
			scheduleBrowseGhostDelay()
			return
		}

		clearBrowseGhostDelayTimer()
		browseGhostVisible.value = false
	},
	{ immediate: true },
)

onMounted(async () => {
	contextLoaded.value = !hasAsyncBrowseContext()
	const canRefreshSearchDuringMetadataLoad =
		!hasAsyncBrowseContext() && Object.keys(route.query).length === 0
	const initialSearchRefresh = canRefreshSearchDuringMetadataLoad
		? searchState.refreshSearch()
		: null

	const [nextCategories, nextLoaders, nextGameVersions] = await Promise.all([
		queryClient
			.fetchQuery({
				queryKey: TAG_CATEGORIES_QUERY_KEY,
				queryFn: get_categories,
				staleTime: DISCOVER_METADATA_STALE_MS,
			})
			.catch(handleError),
		queryClient
			.fetchQuery({
				queryKey: TAG_LOADERS_QUERY_KEY,
				queryFn: get_loaders,
				staleTime: DISCOVER_METADATA_STALE_MS,
			})
			.catch(handleError),
		queryClient
			.fetchQuery({
				queryKey: TAG_GAME_VERSIONS_QUERY_KEY,
				queryFn: get_game_versions,
				staleTime: DISCOVER_METADATA_STALE_MS,
			})
			.catch(handleError),
	])
	categories.value = nextCategories ?? []
	loaders.value = nextLoaders ?? []
	availableGameVersions.value = nextGameVersions ?? []
	tagsLoaded.value = true
	await initInstanceContext()
	contextLoaded.value = true
	await (initialSearchRefresh ?? searchState.refreshSearch())
	preloadSelectableProjectTypes()
})

type UnlistenFn = () => void

let isUnmounted = false
let unlistenInstances: UnlistenFn | null = null

onMounted(() => {
	instance_listener(async (event: { event: string; instance_id: string }) => {
		if (instance.value && event.instance_id === instance.value.id && event.event === 'synced') {
			await refreshInstalledProjectIds()
			await searchState.refreshSearch()
		}
	})
		.then((unlisten) => {
			if (isUnmounted) {
				unlisten()
				return
			}

			unlistenInstances = unlisten
		})
		.catch(handleError)
})

onUnmounted(() => {
	isUnmounted = true
	clearBrowseGhostDelayTimer()
	unlistenInstances?.()
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
	get: () => themeStore.getFeatureFlag('advanced_filters_collapsed'),
	set: (value) => {
		themeStore.featureFlags['advanced_filters_collapsed'] = value
		getSettings()
			.then((settings) => {
				settings.feature_flags['advanced_filters_collapsed'] = value
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
	transitioning: browseTransitioning,
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
	<div
		class="browse-page relative flex flex-col gap-3 p-6"
		:class="{
			'browse-page--ghost': showBrowseGhost,
			'browse-page--transitioning': browseTransitioning,
		}"
		:data-browse-transition-direction="browseTransitionDirection"
	>
		<AppBrowsePageGhost v-if="showBrowseGhost" class="browse-page-ghost-frame" />
		<AppBrowsePageLayout
			v-if="hasMountedBrowseLayout"
			v-show="showBrowseLayout"
			v-model:transitioning="browseTransitioning"
			v-model:transition-direction="browseTransitionDirection"
			:inert="browseTransitioning ? true : undefined"
		>
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
		<div v-if="browseTransitioning" class="browse-page-transition-lock" aria-hidden="true"></div>
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
		<Teleport defer to="#sidebar-teleport-target">
			<div
				class="browse-sidebar-transition-frame"
				:class="{ 'browse-sidebar-transition-frame--transitioning': browseTransitioning }"
				:data-browse-transition-direction="browseTransitionDirection"
				:inert="browseTransitioning ? true : undefined"
			>
				<div class="browse-sidebar-transition-content">
					<BrowseSidebar />
				</div>
				<div
					v-if="browseTransitioning"
					class="browse-sidebar-transition-lock"
					aria-hidden="true"
				></div>
			</div>
		</Teleport>
	</div>
</template>

<style scoped>
.browse-page--ghost {
	box-sizing: border-box;
	height: 100%;
	max-height: calc(100vh - var(--top-bar-height));
	min-height: 0;
	overflow: hidden;
}

.browse-page-ghost-frame {
	flex: 1 1 auto;
	height: 100%;
	min-height: 0;
}

.browse-page-transition-lock {
	position: absolute;
	inset: 0;
	z-index: 9999;
	pointer-events: auto;
	background: transparent;
}

.browse-sidebar-transition-frame {
	position: relative;
	min-height: 100%;
}

.browse-sidebar-transition-content {
	min-height: 100%;
	transition: opacity 120ms cubic-bezier(0.4, 0, 0.2, 1);
}

.browse-sidebar-transition-frame--transitioning .browse-sidebar-transition-content {
	opacity: 0.45;
}

.browse-sidebar-transition-lock {
	position: absolute;
	inset: 0;
	z-index: 2;
	pointer-events: auto;
	background: transparent;
}

@media (prefers-reduced-motion: reduce) {
	.browse-sidebar-transition-content {
		transition-duration: 1ms;
	}
}
</style>
