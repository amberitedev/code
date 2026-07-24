import type { Labrinth } from '@modrinth/api-client'
import type { ComputedRef, Ref, ShallowRef } from 'vue'
import { computed, nextTick, onScopeDispose, ref, shallowRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useDebugLogger } from '#ui/composables/debug-logger'
import type {
	EnvironmentSearchOverride,
	FilterType,
	FilterValue,
	ProjectType,
	SortType,
} from '#ui/utils/search'
import { LOADER_FILTER_TYPES, useSearch } from '#ui/utils/search'
import { useServerSearch } from '#ui/utils/server-search'

import type { BrowseSearchResponse } from '../types'

export interface UseBrowseSearchOptions {
	projectType: Ref<string>
	tags: Ref<{
		gameVersions: Labrinth.Tags.v2.GameVersion[]
		loaders: Labrinth.Tags.v2.Loader[]
		categories: Labrinth.Tags.v2.Category[]
	}>
	providedFilters?: ComputedRef<FilterValue[]>
	environmentOverride?: ComputedRef<EnvironmentSearchOverride | undefined>
	active?: ComputedRef<boolean>
	search: (params: string) => Promise<BrowseSearchResponse>
	persistentQueryParams: string[]
	getExtraQueryParams?: () => Record<string, string | undefined>
	maxResultsOptions?: ComputedRef<number[]>
	displayMode?: Ref<'list' | 'grid' | 'gallery'> | ComputedRef<'list' | 'grid' | 'gallery'>
	immediateProjectTypeSearch?: boolean
	getCachedSearchResponse?: (
		requestParams: string,
		projectType: string,
	) => BrowseSearchResponse | undefined
}

export interface BrowseSearchState {
	query: Ref<string>

	filters: ComputedRef<FilterType[]>
	currentFilters: Ref<FilterValue[]>
	toggledGroups: Ref<string[]>
	overriddenProvidedFilterTypes: Ref<string[]>

	serverFilterTypes: ComputedRef<FilterType[]>
	serverCurrentFilters: Ref<FilterValue[]>
	serverToggledGroups: Ref<string[]>

	effectiveSortTypes: ComputedRef<readonly SortType[]>
	effectiveCurrentSortType: Ref<SortType>

	loading: Ref<boolean>
	projectHits: ShallowRef<BrowseSearchResponse['projectHits']>
	serverHits: ShallowRef<BrowseSearchResponse['serverHits']>
	totalHits: Ref<number>
	activeResultKey: ComputedRef<string>
	visibleResultKey: Ref<string>
	visibleProjectType: Ref<string>
	pageCount: ComputedRef<number>

	maxResults: Ref<number>
	currentPage: Ref<number>

	isServerType: ComputedRef<boolean>
	effectiveLayout: ComputedRef<'list' | 'grid'>
	deprioritizedTags: ComputedRef<string[]>
	excludeLoaders: ComputedRef<boolean>

	refreshSearch: () => Promise<void>
	cacheSearchResponse: (
		projectType: string,
		requestParams: string,
		response: BrowseSearchResponse,
	) => void
	setPage: (page: number) => Promise<void>
	clearSearch: () => void
	onFilterChange: () => void
}

export function useBrowseSearch(options: UseBrowseSearchOptions): BrowseSearchState {
	const debug = useDebugLogger('BrowseSearch')
	const route = useRoute()
	const router = useRouter()

	debug('init, projectType:', options.projectType.value)

	const active = computed(() => options.active?.value ?? true)
	const projectTypes = computed(() => [options.projectType.value] as ProjectType[])
	const isServerType = computed(() => options.projectType.value === 'server')

	const {
		query,
		currentSortType,
		currentFilters,
		toggledGroups,
		maxResults,
		currentPage,
		overriddenProvidedFilterTypes,
		filters,
		sortTypes,
		requestParams,
		createPageParams,
	} = useSearch(
		projectTypes,
		options.tags,
		options.providedFilters ?? computed(() => []),
		options.environmentOverride ?? computed(() => undefined),
	)

	const {
		serverCurrentSortType,
		serverCurrentFilters,
		serverToggledGroups,
		serverSortTypes,
		serverFilterTypes,
		serverRequestParams,
		createServerPageParams,
	} = useServerSearch({
		tags: options.tags,
		query,
		maxResults,
		currentPage,
		providedFilters: options.providedFilters,
	})

	const effectiveRequestParams = computed(() =>
		isServerType.value ? serverRequestParams.value : requestParams.value,
	)
	const effectiveSortTypes = computed(() =>
		isServerType.value ? (serverSortTypes as readonly SortType[]) : sortTypes,
	)
	const effectiveCurrentSortType = computed({
		get: () => (isServerType.value ? serverCurrentSortType.value : currentSortType.value),
		set: (v: SortType) => {
			if (isServerType.value) serverCurrentSortType.value = v
			else currentSortType.value = v
		},
	})

	const effectiveMaxResultsOptions = computed(
		() => options.maxResultsOptions?.value ?? [5, 10, 15, 20, 50, 100],
	)

	watch(effectiveMaxResultsOptions, (opts) => {
		if (!opts.includes(maxResults.value)) {
			maxResults.value = opts.reduce((prev, curr) =>
				Math.abs(curr - maxResults.value) <= Math.abs(prev - maxResults.value) ? curr : prev,
			)
		}
	})

	const effectiveDisplayMode = computed(() => options.displayMode?.value ?? 'list')
	const effectiveLayout = computed<'list' | 'grid'>(() =>
		effectiveDisplayMode.value === 'grid' || effectiveDisplayMode.value === 'gallery'
			? 'grid'
			: 'list',
	)

	const selectedFilterTags = computed(() =>
		currentFilters.value
			.filter(
				(f) =>
					f.type.startsWith('category_') ||
					LOADER_FILTER_TYPES.includes(f.type as (typeof LOADER_FILTER_TYPES)[number]),
			)
			.map((f) => f.option),
	)
	const excludeLoaders = computed(
		() =>
			currentFilters.value.some((f) =>
				LOADER_FILTER_TYPES.includes(f.type as (typeof LOADER_FILTER_TYPES)[number]),
			) || ['resourcepack', 'datapack'].includes(options.projectType.value),
	)
	const loadersNotForThisType = computed(
		() =>
			options.tags.value?.loaders
				?.filter((loader) => !loader.supported_project_types.includes(options.projectType.value))
				?.map((loader) => loader.name) ?? [],
	)
	const deprioritizedTags = computed(() => [
		...selectedFilterTags.value,
		...loadersNotForThisType.value,
	])

	const loading = ref(true)
	const projectHits = shallowRef<BrowseSearchResponse['projectHits']>([])
	const serverHits = shallowRef<BrowseSearchResponse['serverHits']>([])
	const totalHits = ref(0)
	const activeResultKey = computed(() =>
		createSnapshotKey(options.projectType.value, effectiveRequestParams.value),
	)
	const visibleResultKey = ref(activeResultKey.value)
	const visibleProjectType = ref(options.projectType.value)

	const pageCount = computed(() => {
		if (totalHits.value === 0) return 1
		return Math.ceil(totalHits.value / maxResults.value)
	})

	type SearchSnapshot = {
		projectHits: BrowseSearchResponse['projectHits']
		serverHits: BrowseSearchResponse['serverHits']
		totalHits: number
	}

	const MAX_SNAPSHOT_COUNT = 24
	const searchSnapshots = new Map<string, SearchSnapshot>()
	let searchVersion = 0
	let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null
	let lastProjectType = options.projectType.value
	let disposed = false

	function clearSearchDebounce() {
		if (searchDebounceTimer) {
			clearTimeout(searchDebounceTimer)
			searchDebounceTimer = null
		}
	}

	const providedFiltersOrEmpty = computed(() => options.providedFilters?.value ?? [])

	function createSnapshotKey(projectType: string, requestParams: string) {
		return `${projectType}:${requestParams}`
	}

	function setSearchSnapshot(key: string, response: BrowseSearchResponse) {
		if (searchSnapshots.has(key)) {
			searchSnapshots.delete(key)
		}

		searchSnapshots.set(key, {
			projectHits: response.projectHits,
			serverHits: response.serverHits,
			totalHits: response.total_hits,
		})

		if (searchSnapshots.size > MAX_SNAPSHOT_COUNT) {
			const oldestKey = searchSnapshots.keys().next().value
			if (oldestKey) {
				searchSnapshots.delete(oldestKey)
			}
		}
	}

	function restoreSearchSnapshot(
		key = activeResultKey.value,
		projectType = options.projectType.value,
	) {
		const snapshot = searchSnapshots.get(key)
		if (!snapshot) return false

		projectHits.value = snapshot.projectHits
		serverHits.value = snapshot.serverHits
		totalHits.value = snapshot.totalHits
		visibleResultKey.value = key
		visibleProjectType.value = projectType
		loading.value = false
		return true
	}

	function restoreCachedSearchResponse(
		requestParams = effectiveRequestParams.value,
		projectType = options.projectType.value,
		key = createSnapshotKey(projectType, requestParams),
	) {
		const response = options.getCachedSearchResponse?.(requestParams, projectType)
		if (!response) return false

		setSearchSnapshot(key, response)
		return restoreSearchSnapshot(key, projectType)
	}

	restoreCachedSearchResponse()

	watch(
		[
			query,
			maxResults,
			options.projectType,
			currentSortType,
			serverCurrentSortType,
			currentFilters,
			serverCurrentFilters,
			overriddenProvidedFilterTypes,
			providedFiltersOrEmpty,
		],
		() => {
			currentPage.value = 1
		},
		{ deep: true, flush: 'sync' },
	)

	watch(
		() => options.projectType.value,
		(newType, oldType) => {
			debug('projectType changed', { from: oldType, to: newType })
			effectiveCurrentSortType.value =
				effectiveSortTypes.value.find((sortType) => sortType.name === 'relevance') ??
				effectiveSortTypes.value[0]
			query.value = ''
		},
		{ flush: 'sync' },
	)

	watch(
		effectiveRequestParams,
		(newVal, oldVal) => {
			debug('effectiveRequestParams changed', {
				from: oldVal?.substring(0, 80),
				to: newVal?.substring(0, 80),
			})
			const restored = restoreSearchSnapshot() || restoreCachedSearchResponse()
			if (searchDebounceTimer) clearTimeout(searchDebounceTimer)
			const projectTypeChanged = lastProjectType !== options.projectType.value
			lastProjectType = options.projectType.value

			if (options.immediateProjectTypeSearch && projectTypeChanged) {
				if (!restored) void refreshSearch()
				else updateUrlParams()
				return
			}

			if (restored) {
				updateUrlParams()
				return
			}

			searchDebounceTimer = setTimeout(() => {
				refreshSearch()
			}, 200)
		},
		{ flush: 'sync' },
	)

	watch(active, (isActive, wasActive) => {
		clearSearchDebounce()
		if (isActive && wasActive === false) {
			void refreshSearch()
		}
	})

	async function refreshSearch() {
		if (!active.value) {
			return
		}

		const version = ++searchVersion
		const requestProjectType = options.projectType.value
		const requestParams = effectiveRequestParams.value
		const requestKey = createSnapshotKey(requestProjectType, requestParams)
		const requestIsServerType = requestProjectType === 'server'
		const hasCachedSnapshot = searchSnapshots.has(requestKey)
		debug('refreshSearch start', {
			version,
			projectType: requestProjectType,
			params: requestParams.substring(0, 100),
		})

		const currentHitsEmpty = requestIsServerType
			? serverHits.value.length === 0
			: projectHits.value.length === 0
		if (currentHitsEmpty && !hasCachedSnapshot && visibleResultKey.value === requestKey) {
			loading.value = true
		}

		try {
			const response = await options.search(requestParams)
			setSearchSnapshot(requestKey, response)

			if (disposed || version !== searchVersion) {
				debug('refreshSearch stale, discarding', { version, current: searchVersion })
				return
			}

			if (requestIsServerType) {
				serverHits.value = response.serverHits
			} else {
				projectHits.value = response.projectHits
			}
			totalHits.value = response.total_hits
			visibleResultKey.value = requestKey
			visibleProjectType.value = requestProjectType
			debug('refreshSearch complete', {
				version,
				hits: response.total_hits,
				projectHits: response.projectHits.length,
				serverHits: response.serverHits.length,
			})

			updateUrlParams()
			loading.value = false
		} catch (err) {
			debug('refreshSearch error', err)
			console.error('Browse search error:', err)
			if (version === searchVersion) {
				if (requestIsServerType) {
					serverHits.value = []
				} else {
					projectHits.value = []
				}
				totalHits.value = 0
				visibleResultKey.value = requestKey
				visibleProjectType.value = requestProjectType
				loading.value = false
			}
		}
	}

	function cacheSearchResponse(
		projectType: string,
		requestParams: string,
		response: BrowseSearchResponse,
	) {
		setSearchSnapshot(createSnapshotKey(projectType, requestParams), response)
	}

	function updateUrlParams() {
		if (disposed) return

		debug('updateUrlParams', { path: route.path })
		const persistentParams: Record<string, string | (string | null)[] | null | undefined> = {}

		for (const [key, value] of Object.entries(route.query)) {
			if (options.persistentQueryParams.includes(key)) {
				persistentParams[key] = value
			}
		}

		const extraParams = options.getExtraQueryParams?.() ?? {}
		for (const [key, value] of Object.entries(extraParams)) {
			persistentParams[key] = value
		}

		const params = {
			...persistentParams,
			...(isServerType.value ? createServerPageParams() : createPageParams()),
		}

		void router.replace({ path: route.path, query: params }).catch((error) => {
			debug('updateUrlParams failed', error)
		})
	}

	async function setPage(newPageNumber: number) {
		currentPage.value = newPageNumber
		await nextTick()
		window.scrollTo({ top: 0, behavior: 'smooth' })
	}

	function clearSearch() {
		query.value = ''
		currentPage.value = 1
	}

	function onFilterChange() {
		nextTick(() => window.scrollTo({ top: 0, behavior: 'smooth' }))
	}

	onScopeDispose(() => {
		disposed = true
		searchVersion++

		if (searchDebounceTimer) {
			clearTimeout(searchDebounceTimer)
			searchDebounceTimer = null
		}
	})

	return {
		query,
		filters,
		currentFilters,
		toggledGroups,
		overriddenProvidedFilterTypes,
		serverFilterTypes,
		serverCurrentFilters,
		serverToggledGroups,
		effectiveSortTypes,
		effectiveCurrentSortType,
		loading,
		projectHits,
		serverHits,
		totalHits,
		activeResultKey,
		visibleResultKey,
		visibleProjectType,
		pageCount,
		maxResults,
		currentPage,
		isServerType,
		effectiveLayout,
		deprioritizedTags,
		excludeLoaders,
		refreshSearch,
		cacheSearchResponse,
		setPage,
		clearSearch,
		onFilterChange,
	}
}
