import type { Labrinth } from '@modrinth/api-client'
import type { ComputedRef, Ref, ShallowRef } from 'vue'
import { computed, nextTick, ref, shallowRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useDebugLogger } from '#ui/composables/debug-logger'
import {
	compatibleAdvancedFilters,
	getAdvancedOptionIds,
	getAvailableAdvancedIds,
	mergeAdvancedPrefs,
	replaceAdvancedFilters,
	sameOptionIds,
	useAdvancedPrefs,
} from '#ui/utils/advanced-filter-preferences'
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
	/**
	 * Returns the results for `params` when they are already cached and fresh. With it, a search
	 * that is cached shows in the same frame as the action that started it.
	 */
	peekSearch?: (params: string) => BrowseSearchResponse | undefined
	/** Caches the results for `params` without showing them. Enables `prefetchProjectType`. */
	prefetchSearch?: (params: string) => void
	/** The provided filters another project type would get, to prefetch its search. */
	providedFiltersFor?: (projectType: string) => FilterValue[]
	persistentQueryParams: string[]
	getExtraQueryParams?: () => Record<string, string | undefined>
	maxResultsOptions?: ComputedRef<number[]>
	displayMode?: Ref<'list' | 'grid' | 'gallery'> | ComputedRef<'list' | 'grid' | 'gallery'>
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
	refreshing: Ref<boolean>
	projectHits: ShallowRef<BrowseSearchResponse['projectHits']>
	serverHits: ShallowRef<BrowseSearchResponse['serverHits']>
	totalHits: Ref<number>
	pageCount: ComputedRef<number>

	maxResults: Ref<number>
	currentPage: Ref<number>

	isServerType: ComputedRef<boolean>
	effectiveLayout: ComputedRef<'list' | 'grid'>
	deprioritizedTags: ComputedRef<string[]>
	excludeLoaders: ComputedRef<boolean>

	refreshSearch: () => Promise<void>
	/** Call when a project type tab is hovered, so its results are usually ready by the click. */
	prefetchProjectType: (projectType: string) => void
	setPage: (page: number) => Promise<void>
	clearSearch: () => void
	onFilterChange: () => void

	linkOverridesAdvancedPrefs: Ref<boolean>
	applySavedAdvancedPrefs: () => void
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
		createFilterTypes,
		createSwitchRequestParams,
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
	const refreshing = ref(false)
	const projectHits = shallowRef<BrowseSearchResponse['projectHits']>([])
	const serverHits = shallowRef<BrowseSearchResponse['serverHits']>([])
	const totalHits = ref(0)

	const pageCount = computed(() => {
		if (totalHits.value === 0) return 1
		return Math.ceil(totalHits.value / maxResults.value)
	})

	let searchVersion = 0
	// The project type the hits on screen were searched for.
	let hitsProjectType: string | undefined
	let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null

	function clearSearchDebounce() {
		if (searchDebounceTimer) {
			clearTimeout(searchDebounceTimer)
			searchDebounceTimer = null
		}
	}

	const providedFiltersOrEmpty = computed(() => options.providedFilters?.value ?? [])
	const effectiveCurrentFilters = computed(() =>
		isServerType.value ? serverCurrentFilters.value : currentFilters.value,
	)
	const effectiveFilterTypes = computed(() =>
		isServerType.value ? serverFilterTypes.value : filters.value,
	)

	const advancedPrefs = useAdvancedPrefs()
	const linkOverridesAdvancedPrefs = ref(false)
	const selectedAdvancedIds = computed(() => getAdvancedOptionIds(effectiveCurrentFilters.value))

	function hasSearchQuery(): boolean {
		return Object.keys(route.query).some((key) => !options.persistentQueryParams.includes(key))
	}

	function getCompatiblePrefs(): FilterValue[] {
		return compatibleAdvancedFilters(advancedPrefs.value, effectiveFilterTypes.value)
	}

	function setEffectiveFilters(nextFilters: FilterValue[]) {
		if (isServerType.value) {
			serverCurrentFilters.value = nextFilters
		} else {
			currentFilters.value = nextFilters
		}
	}

	function applyAdvancedPrefs(prefs: FilterValue[]) {
		setEffectiveFilters(replaceAdvancedFilters(effectiveCurrentFilters.value, prefs))
	}

	function syncLinkOverride() {
		const prefIds = getAdvancedOptionIds(getCompatiblePrefs())
		linkOverridesAdvancedPrefs.value =
			prefIds.length > 0 && !sameOptionIds(selectedAdvancedIds.value, prefIds)
	}

	function initAdvancedPrefs() {
		const prefs = getCompatiblePrefs()

		if (!hasSearchQuery()) {
			if (prefs.length > 0) {
				applyAdvancedPrefs(prefs)
			}
			linkOverridesAdvancedPrefs.value = false
			return
		}

		syncLinkOverride()
	}

	function applySavedAdvancedPrefs() {
		applyAdvancedPrefs(getCompatiblePrefs())
		linkOverridesAdvancedPrefs.value = false
	}

	initAdvancedPrefs()

	watch(selectedAdvancedIds, (selected, previous) => {
		if (previous && sameOptionIds(selected, previous)) {
			return
		}

		const nextPrefs = mergeAdvancedPrefs(
			advancedPrefs.value,
			selected,
			getAvailableAdvancedIds(effectiveFilterTypes.value),
		)
		if (!sameOptionIds(advancedPrefs.value, nextPrefs)) {
			advancedPrefs.value = nextPrefs
		}
		linkOverridesAdvancedPrefs.value = false
	})

	watch(
		[
			query,
			maxResults,
			options.projectType,
			effectiveCurrentSortType,
			effectiveCurrentFilters,
			overriddenProvidedFilterTypes,
			providedFiltersOrEmpty,
		],
		() => {
			currentPage.value = 1
		},
		{ deep: true },
	)

	// Declared before the request watcher so a project type switch settles the sort, query and
	// advanced filters first, and the switch then makes exactly one search.
	watch(
		() => options.projectType.value,
		(newType, oldType) => {
			debug('projectType changed', { from: oldType, to: newType })
			effectiveCurrentSortType.value =
				effectiveSortTypes.value.find((sortType) => sortType.name === 'relevance') ??
				effectiveSortTypes.value[0]
			query.value = ''
			initAdvancedPrefs()
		},
	)

	let lastQuery = query.value

	watch(effectiveRequestParams, (newVal, oldVal) => {
		refreshing.value = true
		debug('effectiveRequestParams changed', {
			from: oldVal?.substring(0, 80),
			to: newVal?.substring(0, 80),
		})
		clearSearchDebounce()
		const typing = query.value !== lastQuery && query.value !== ''
		lastQuery = query.value
		if (!active.value) {
			return
		}
		// Only typing waits for a pause. A project type, filter, sort or page change is one action,
		// so it searches before the next render instead of showing the old results for 200ms.
		if (typing) {
			searchDebounceTimer = setTimeout(() => {
				refreshSearch()
			}, 200)
		} else {
			void refreshSearch()
		}
	})

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
		refreshing.value = true
		debug('refreshSearch start', {
			version,
			projectType: options.projectType.value,
			params: effectiveRequestParams.value.substring(0, 100),
		})

		const cached = options.peekSearch?.(effectiveRequestParams.value)
		if (cached) {
			showResponse(cached)
			return
		}

		// Another project type's results are never shown under this one: the ghosts take their place.
		if (hitsProjectType !== options.projectType.value) {
			projectHits.value = []
			serverHits.value = []
		}

		const currentHitsEmpty = isServerType.value
			? serverHits.value.length === 0
			: projectHits.value.length === 0
		if (currentHitsEmpty) {
			loading.value = true
		}

		try {
			const response = await options.search(effectiveRequestParams.value)

			if (!active.value) {
				return
			}

			if (version !== searchVersion) {
				debug('refreshSearch stale, discarding', { version, current: searchVersion })
				return
			}

			showResponse(response)
			debug('refreshSearch complete', {
				version,
				hits: response.total_hits,
				projectHits: response.projectHits.length,
				serverHits: response.serverHits.length,
			})
		} catch (err) {
			debug('refreshSearch error', err)
			console.error('Browse search error:', err)
			if (version === searchVersion) {
				loading.value = false
				refreshing.value = false
			}
		}
	}

	function showResponse(response: BrowseSearchResponse) {
		if (isServerType.value) {
			serverHits.value = response.serverHits
		} else {
			projectHits.value = response.projectHits
		}
		hitsProjectType = options.projectType.value
		totalHits.value = response.total_hits

		updateUrlParams()
		loading.value = false
		refreshing.value = false
	}

	function prefetchProjectType(projectType: string) {
		if (!options.prefetchSearch || !active.value || projectType === options.projectType.value) {
			return
		}
		const params = getSwitchRequestParams(projectType)
		if (params) {
			options.prefetchSearch(params)
		}
	}

	// Mirrors what the project type watcher does on a switch, without switching.
	function getSwitchRequestParams(projectType: string): string | undefined {
		const toServer = projectType === 'server'
		const filterTypes = toServer
			? serverFilterTypes.value
			: createFilterTypes([projectType as ProjectType])
		const selected = toServer ? serverCurrentFilters.value : currentFilters.value
		const prefs = compatibleAdvancedFilters(advancedPrefs.value, filterTypes)
		const switchFilters = prefs.length > 0 ? replaceAdvancedFilters(selected, prefs) : selected
		const provided = options.providedFiltersFor?.(projectType) ?? providedFiltersOrEmpty.value

		if (toServer) {
			// The server request cannot be built ahead, so it is only prefetched when a switch would
			// leave it as it is now. Provided project ids are the only provided filters it reads.
			const unchanged =
				query.value === '' &&
				currentPage.value === 1 &&
				serverCurrentSortType.value.name === 'relevance' &&
				sameOptionIds(getAdvancedOptionIds(selected), getAdvancedOptionIds(switchFilters)) &&
				![...provided, ...providedFiltersOrEmpty.value].some((f) => f.type === 'project_id')
			return unchanged ? serverRequestParams.value : undefined
		}

		return createSwitchRequestParams(
			[projectType as ProjectType],
			filterTypes,
			switchFilters,
			provided,
		)
	}

	function updateUrlParams() {
		if (!active.value) {
			return
		}

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

		router.replace({ path: route.path, query: params })
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
		refreshing,
		projectHits,
		serverHits,
		totalHits,
		pageCount,
		maxResults,
		currentPage,
		isServerType,
		effectiveLayout,
		deprioritizedTags,
		excludeLoaders,
		refreshSearch,
		prefetchProjectType,
		setPage,
		clearSearch,
		onFilterChange,
		linkOverridesAdvancedPrefs,
		applySavedAdvancedPrefs,
	}
}
