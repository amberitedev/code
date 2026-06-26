import type { QueryClient, QueryKey } from '@tanstack/vue-query'
import type { MaybeRefOrGetter } from 'vue'
import { computed, onBeforeUnmount, ref, toValue, watch } from 'vue'

type PreloadQueryOptions<TData> = {
	queryKey: QueryKey
	queryFn: () => TData | Promise<TData>
	staleTime?: number
	gcTime?: number
}

type OptimisticLoadingOptions = {
	delay?: number
	minimumVisible?: number
}

const DEFAULT_GHOST_DELAY_MS = 1
const DEFAULT_MINIMUM_VISIBLE_MS = 180

export function preloadQuery<TData>(
	queryClient: QueryClient,
	options: PreloadQueryOptions<TData>,
): Promise<void> {
	const state = queryClient.getQueryState(options.queryKey)
	if (state?.fetchStatus === 'fetching' || state?.data !== undefined) {
		return Promise.resolve()
	}

	return queryClient.prefetchQuery(options).catch(() => undefined)
}

export function preloadQueries(
	queryClient: QueryClient,
	queries: Array<PreloadQueryOptions<unknown>>,
): void {
	for (const query of queries) {
		void preloadQuery(queryClient, query)
	}
}

export function useOptimisticLoading(
	pending: MaybeRefOrGetter<boolean>,
	hasContent: MaybeRefOrGetter<boolean>,
	options: OptimisticLoadingOptions = {},
) {
	const visible = ref(false)
	const delay = options.delay ?? DEFAULT_GHOST_DELAY_MS
	const minimumVisible = options.minimumVisible ?? DEFAULT_MINIMUM_VISIBLE_MS
	let showTimer: ReturnType<typeof setTimeout> | null = null
	let hideTimer: ReturnType<typeof setTimeout> | null = null
	let shownAt = 0

	function clearShowTimer() {
		if (showTimer !== null) {
			clearTimeout(showTimer)
			showTimer = null
		}
	}

	function clearHideTimer() {
		if (hideTimer !== null) {
			clearTimeout(hideTimer)
			hideTimer = null
		}
	}

	function hide() {
		clearHideTimer()
		visible.value = false
	}

	function sync() {
		const shouldShow = toValue(pending) && !toValue(hasContent)

		if (!shouldShow) {
			clearShowTimer()

			if (!visible.value) {
				return
			}

			const remainingVisibleMs = minimumVisible - (Date.now() - shownAt)
			if (remainingVisibleMs > 0) {
				clearHideTimer()
				hideTimer = setTimeout(hide, remainingVisibleMs)
			} else {
				hide()
			}
			return
		}

		clearHideTimer()
		if (visible.value || showTimer !== null) {
			return
		}

		showTimer = setTimeout(() => {
			showTimer = null

			if (toValue(pending) && !toValue(hasContent)) {
				shownAt = Date.now()
				visible.value = true
			}
		}, delay)
	}

	watch(() => [toValue(pending), toValue(hasContent)], sync, { immediate: true })

	onBeforeUnmount(() => {
		clearShowTimer()
		clearHideTimer()
	})

	return computed(() => visible.value)
}
