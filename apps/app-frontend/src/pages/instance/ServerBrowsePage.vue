<template>
	<div
		data-library-instance-page-ready
		data-library-instance-drag-disabled
		:data-library-instance-title="serverName"
		:data-library-instance-subtitle="serverSubtitle"
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
		/>
		<div v-if="browseTransitioning" class="browse-page-transition-lock" aria-hidden="true"></div>
	</div>
</template>

<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { CheckIcon, PlusIcon, SpinnerIcon } from '@modrinth/assets'
import {
	commonMessages,
	defineMessages,
	injectNotificationManager,
	provideBrowseManager,
	type BrowseInstallContext,
	type BrowseSearchResponse,
	type CardAction,
	type FilterValue,
	type ProjectType,
	type Tags,
	type UiMotionDirection,
	useBrowseSearch,
	useVIntl,
} from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, onUnmounted, ref, type Ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import AppBrowsePageGhost from '@/components/ui/AppBrowsePageGhost.vue'
import AppBrowsePageLayout from '@/components/ui/AppBrowsePageLayout.vue'
import {
	buildDiscoverSearchParams,
	DISCOVER_PRELOAD_STALE_MS,
} from '@/composables/useDiscoverContentPreload'
import { useCoreClient } from '@/composables/useCoreClient'
import { useCoreInstances } from '@/composables/useCoreInstances'

const MOD_LOADERS = ['fabric', 'forge', 'quilt', 'neoforge']
const PLUGIN_LOADERS = ['paper', 'purpur']
const BROWSE_GHOST_DELAY_MS = 80
const MODRINTH_API_BASE = 'https://api.modrinth.com'

const messages = defineMessages({
	backToContent: {
		id: 'core-server.browse.back-to-content',
		defaultMessage: 'Back to content',
	},
	installing: {
		id: 'core-server.browse.installing',
		defaultMessage: 'Installing...',
	},
	added: {
		id: 'core-server.browse.added',
		defaultMessage: '{title} was added to the server.',
	},
	serverProfileType: {
		id: 'core-server.browse.profile-type.server',
		defaultMessage: 'Server',
	},
	pluginsProjectType: {
		id: 'core-server.browse.project-type.plugins',
		defaultMessage: 'Plugins',
	},
	gameVersionProvidedByServer: {
		id: 'core-server.browse.game-version-provided',
		defaultMessage: 'Game version is provided by the server',
	},
	modLoaderProvidedByServer: {
		id: 'core-server.browse.loader-provided',
		defaultMessage: 'Loader is provided by the server',
	},
	environmentProvidedByServer: {
		id: 'core-server.browse.environment-provided',
		defaultMessage: 'Environment is provided by the server',
	},
	providedByServer: {
		id: 'core-server.browse.provided-by',
		defaultMessage: 'Provided by the server',
	},
})

const route = useRoute()
const router = useRouter()
const core = useCoreClient()
const queryClient = useQueryClient()
const { instances: coreInstances } = useCoreInstances()
const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()

const serverId = computed(() => route.params.id as string)
const backPath = computed(() => `/instance/${encodeURIComponent(serverId.value)}/content`)
const cachedServer = computed(() => {
	const instances = [...coreInstances.value.values()]
	return instances.find(
		(instance) => instance.path === serverId.value || instance.id === serverId.value,
	)
})

const serverQuery = useQuery({
	queryKey: computed(() => ['core-server', serverId.value]),
	queryFn: () => core.getInstance(serverId.value),
	staleTime: 30_000,
	retry: false,
})

const server = computed(() => serverQuery.data.value ?? cachedServer.value ?? null)
const serverName = computed(() => server.value?.name ?? serverId.value)
const serverLoader = computed(() => server.value?.loader ?? 'fabric')
const serverGameVersion = computed(() => server.value?.game_version ?? '')
const serverSubtitle = computed(() =>
	[formatLoader(serverLoader.value), serverGameVersion.value].filter(Boolean).join(' '),
)
const projectType = ref<ProjectType>(getRouteProjectType() ?? getDefaultProjectType(serverLoader.value))

const categories = ref<Labrinth.Tags.v2.Category[]>([])
const loaders = ref<Labrinth.Tags.v2.Loader[]>([])
const gameVersions = ref<Labrinth.Tags.v2.GameVersion[]>([])
const tagsLoaded = ref(false)
const installedProjectIdsLoaded = ref(false)
const installing = ref<Set<string>>(new Set())
const installedProjectIds = ref<Set<string>>(new Set())
const browseTransitioning = ref(false)
const browseTransitionDirection = ref<UiMotionDirection>('forward')

const tags = computed<Tags>(() => ({
	gameVersions: gameVersions.value,
	loaders: loaders.value,
	categories: categories.value,
})) as unknown as Ref<Tags>

const selectableProjectTypes = computed(() =>
	getSelectableProjectTypes(serverLoader.value).map((type) => createBrowseTab(type)),
)

const providedFilters = computed<FilterValue[]>(() => {
	const filters: FilterValue[] = []
	const loader = serverLoader.value.toLowerCase()

	if (serverGameVersion.value) {
		filters.push({ type: 'game_version', option: serverGameVersion.value })
	}

	if (projectType.value === 'mod' && MOD_LOADERS.includes(loader)) {
		filters.push({ type: 'mod_loader', option: loader })
		filters.push({ type: 'environment', option: 'server' })
	}

	if (projectType.value === 'plugin' && PLUGIN_LOADERS.includes(loader)) {
		filters.push({ type: 'plugin_loader', option: loader })
	}

	return filters
})

const installContext = computed<BrowseInstallContext>(() => ({
	name: serverName.value,
	loader: serverLoader.value,
	gameVersion: serverGameVersion.value,
	profileTypeLabel: formatMessage(messages.serverProfileType),
	backUrl: backPath.value,
	backLabel: formatMessage(messages.backToContent),
	heading: formatMessage(commonMessages.installingContentLabel),
	onBack: () => {
		void router.push(backPath.value)
		return false
	},
}))

async function loadModrinth<T>(path: string): Promise<T> {
	const response = await fetch(`${MODRINTH_API_BASE}${path}`)
	if (!response.ok) {
		throw new Error(`Modrinth request failed: ${response.status} ${response.statusText}`)
	}
	return (await response.json()) as T
}

async function loadTags() {
	try {
		const [nextCategories, nextLoaders, nextGameVersions] = await Promise.all([
			loadModrinth<Labrinth.Tags.v2.Category[]>('/v2/tag/category'),
			loadModrinth<Labrinth.Tags.v2.Loader[]>('/v2/tag/loader'),
			loadModrinth<Labrinth.Tags.v2.GameVersion[]>('/v2/tag/game_version'),
		])
		categories.value = nextCategories
		loaders.value = nextLoaders
		gameVersions.value = nextGameVersions
	} catch (err) {
		handleError(err as Error)
	} finally {
		tagsLoaded.value = true
	}
}

async function refreshInstalledProjectIds() {
	const mods = await core.listMods(serverId.value)
	installedProjectIds.value = new Set(
		mods
			.map((mod) => mod.modrinth_project_id)
			.filter((id): id is string => typeof id === 'string' && id.length > 0),
	)
}

type RawSearchResults =
	| (Labrinth.Search.v3.SearchResults & {
			hits: Labrinth.Search.v3.ResultSearchProject[]
	  })
	| {
			result: Labrinth.Search.v3.SearchResults & {
				hits: Labrinth.Search.v3.ResultSearchProject[]
			}
	  }

function createBrowseSearchResponse(
	raw: RawSearchResults,
	searchProjectType: ProjectType,
): BrowseSearchResponse {
	const result = 'result' in raw ? raw.result : raw
	const projectHits = result.hits.map((hit) => {
		const projectId = hit.project_id ?? hit.id ?? hit.slug ?? ''
		return {
			...hit,
			project_id: projectId,
			title: hit.name ?? hit.title,
			description: hit.summary ?? hit.description,
			icon_url: hit.icon_url ?? null,
			project_type: hit.project_type ?? searchProjectType,
			slug: hit.slug ?? projectId,
			installed: projectId ? installedProjectIds.value.has(projectId) : false,
		} as unknown as Labrinth.Search.v2.ResultSearchProject & {
			installed?: boolean
			installing?: boolean
		}
	})

	return {
		projectHits,
		serverHits: [],
		total_hits: result.total_hits ?? 0,
		per_page: result.hits_per_page ?? 20,
	}
}

async function search(requestParams: string): Promise<BrowseSearchResponse> {
	const searchProjectType = projectType.value
	const suffix = requestParams.startsWith('?') ? requestParams : `?${requestParams}`
	const raw = await loadModrinth<RawSearchResults>(`/v3/search${suffix}`)
	return createBrowseSearchResponse(raw, searchProjectType)
}

const searchState = useBrowseSearch({
	projectType,
	tags,
	providedFilters,
	search,
	immediateProjectTypeSearch: true,
	persistentQueryParams: ['type'],
	getExtraQueryParams: () => ({
		type: projectType.value,
	}),
})

const browseInitialPending = computed(
	() =>
		!tagsLoaded.value ||
		!installedProjectIdsLoaded.value ||
		serverQuery.isLoading.value ||
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

watch([serverLoader, () => route.query.type], () => {
	const routeProjectType = getRouteProjectType()
	const availableProjectTypes = getSelectableProjectTypes(serverLoader.value)
	const nextProjectType =
		routeProjectType && availableProjectTypes.includes(routeProjectType)
			? routeProjectType
			: availableProjectTypes.includes(projectType.value)
				? projectType.value
				: getDefaultProjectType(serverLoader.value)

	if (projectType.value !== nextProjectType) {
		projectType.value = nextProjectType
	}
})

watch(serverId, async () => {
	installedProjectIdsLoaded.value = false
	try {
		await refreshInstalledProjectIds()
	} catch (err) {
		handleError(err as Error)
	} finally {
		installedProjectIdsLoaded.value = true
	}
	await searchState.refreshSearch()
})

function createBrowseTab(type: ProjectType) {
	return {
		label: getProjectTypeLabel(type),
		href: `${backPath.value.replace(/\/content$/, '/browse')}?type=${type}`,
		onHover: () => preloadProjectType(type),
	}
}

function getRouteProjectType() {
	const value = Array.isArray(route.query.type) ? route.query.type[0] : route.query.type
	return isServerBrowseProjectType(value) ? value : null
}

function isServerBrowseProjectType(value: unknown): value is ProjectType {
	return (
		value === 'mod' ||
		value === 'plugin' ||
		value === 'resourcepack' ||
		value === 'datapack' ||
		value === 'shader'
	)
}

function getSelectableProjectTypes(loader: string): ProjectType[] {
	const normalized = loader.toLowerCase()
	const types: ProjectType[] = []

	if (MOD_LOADERS.includes(normalized)) {
		types.push('mod')
	}

	if (PLUGIN_LOADERS.includes(normalized)) {
		types.push('plugin')
	}

	types.push('resourcepack', 'datapack', 'shader')
	return types
}

function getDefaultProjectType(loader: string): ProjectType {
	const normalized = loader.toLowerCase()
	if (PLUGIN_LOADERS.includes(normalized)) return 'plugin'
	if (MOD_LOADERS.includes(normalized)) return 'mod'
	return 'datapack'
}

function getProjectTypeLabel(type: ProjectType) {
	if (type === 'mod') return 'Mods'
	if (type === 'plugin') return formatMessage(messages.pluginsProjectType)
	if (type === 'resourcepack') return 'Resource Packs'
	if (type === 'datapack') return 'Data Packs'
	if (type === 'shader') return 'Shaders'
	return type
}

function preloadProjectType(type: ProjectType) {
	if (!tagsLoaded.value || type === projectType.value) return

	const requestParams = buildDiscoverSearchParams(type, searchState.maxResults.value)
	void queryClient
		.prefetchQuery({
			queryKey: ['search', 'v3', requestParams],
			queryFn: () => loadModrinth<RawSearchResults>(`/v3/search${requestParams}`),
			staleTime: DISCOVER_PRELOAD_STALE_MS,
		})
		.catch(() => undefined)
}

function preloadSelectableProjectTypes() {
	for (const link of selectableProjectTypes.value) {
		const type = getProjectTypeFromTabHref(link.href)
		if (type) preloadProjectType(type)
	}
}

function getProjectTypeFromTabHref(href: string) {
	try {
		const type = new URL(href, 'https://app.local').searchParams.get('type')
		return isServerBrowseProjectType(type) ? type : null
	} catch {
		return null
	}
}

function getProjectBrowseQuery() {
	return {
		...route.query,
		b: route.fullPath,
	}
}

function getCardActions(
	result: Labrinth.Search.v2.ResultSearchProject | Labrinth.Search.v3.ResultSearchProject,
): CardAction[] {
	const projectId = result.project_id ?? result.slug
	if (!projectId) return []

	const title =
		(result as Labrinth.Search.v2.ResultSearchProject).title ??
		(result as Labrinth.Search.v3.ResultSearchProject).name ??
		projectId
	const isInstalled = installedProjectIds.value.has(projectId)
	const isInstalling = installing.value.has(projectId)

	return [
		{
			key: 'install',
			label: formatMessage(
				isInstalling
					? messages.installing
					: isInstalled
						? commonMessages.installedLabel
						: commonMessages.installButton,
			),
			icon: isInstalling ? SpinnerIcon : isInstalled ? CheckIcon : PlusIcon,
			iconClass: isInstalling ? 'animate-spin' : undefined,
			disabled: isInstalled || isInstalling,
			color: 'brand',
			type: 'outlined',
			onClick: async () => {
				if (isInstalled || isInstalling) return
				installing.value = new Set(installing.value).add(projectId)
				try {
					await core.addModProject(serverId.value, projectId)
					addNotification({
						title: formatMessage(commonMessages.installedLabel),
						text: formatMessage(messages.added, { title }),
						type: 'success',
					})
					await refreshInstalledProjectIds()
					await queryClient.invalidateQueries({ queryKey: ['core-mods', serverId.value] })
					await searchState.refreshSearch()
				} catch (err) {
					handleError(err as Error)
				} finally {
					const next = new Set(installing.value)
					next.delete(projectId)
					installing.value = next
				}
			},
		},
	]
}

function formatLoader(loader: string) {
	if (loader === 'neoforge') return 'NeoForge'
	return loader.charAt(0).toUpperCase() + loader.slice(1)
}

async function initializeBrowse() {
	await loadTags()
	try {
		await refreshInstalledProjectIds()
	} catch (err) {
		handleError(err as Error)
	} finally {
		installedProjectIdsLoaded.value = true
	}
	await searchState.refreshSearch()
	preloadSelectableProjectTypes()
}

provideBrowseManager({
	...searchState,
	tags,
	projectType,
	transitioning: browseTransitioning,
	variant: 'app',
	providedFilters,
	installContext,
	getCardActions,
	showProjectTypeTabs: computed(() => true),
	selectableProjectTypes,
	getProjectLink: (result) => ({
		path: `/project/${result.project_id ?? result.slug}`,
		query: getProjectBrowseQuery(),
	}),
	getServerProjectLink: (result) => ({
		path: `/project/${result.slug ?? result.project_id}`,
		query: getProjectBrowseQuery(),
	}),
	lockedFilterMessages: {
		gameVersion: formatMessage(messages.gameVersionProvidedByServer),
		modLoader: formatMessage(messages.modLoaderProvidedByServer),
		environment: formatMessage(messages.environmentProvidedByServer),
		providedBy: formatMessage(messages.providedByServer),
	},
})

void initializeBrowse()

onUnmounted(() => {
	clearBrowseGhostDelayTimer()
})
</script>

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
</style>
