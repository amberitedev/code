import { computed, nextTick, ref, shallowRef, toValue, watch, type MaybeRefOrGetter } from 'vue'
import type { RouteLocationRaw, Router } from 'vue-router'

import type { UiMotionDirection } from './ui-motion'

export interface NavTabContentControllerTab {
	href?: RouteLocationRaw
	onHover?: () => void
}

export interface UseNavTabContentControllerOptions<Tab extends NavTabContentControllerTab> {
	activeIndex: MaybeRefOrGetter<number>
	router?: Router
	replace?: boolean
	getLocation?: (tab: Tab, index: number) => RouteLocationRaw | undefined
	isCurrentTab?: (index: number, tab: Tab) => boolean
	changeTab?: (index: number, tab: Tab) => boolean | void | Promise<boolean | void>
}

interface PendingTab<Tab> {
	index: number
	tab: Tab
	token: number
}

function isValidIndex(index: number | undefined) {
	return typeof index === 'number' && index >= 0
}

function getDirection(previous: number, next: number): UiMotionDirection {
	return next > previous ? 'forward' : 'backward'
}

export function useNavTabContentController<Tab extends NavTabContentControllerTab>(
	options: UseNavTabContentControllerOptions<Tab>,
) {
	const actualActiveIndex = computed(() => toValue(options.activeIndex))
	const optimisticActiveIndex = ref<number | null>(null)
	const activeIndex = computed(() => optimisticActiveIndex.value ?? actualActiveIndex.value)
	const direction = ref<UiMotionDirection>('forward')
	const visible = ref(true)
	const pendingTab = shallowRef<PendingTab<Tab> | null>(null)
	const applyingPendingTab = ref(false)
	const pendingChangeApplied = ref(false)
	const leaveFinished = ref(false)

	let token = 0

	function getTabLocation(index: number, tab: Tab) {
		return options.getLocation?.(tab, index) ?? tab.href
	}

	function isCurrentTab(index: number, tab: Tab) {
		if (options.isCurrentTab) return options.isCurrentTab(index, tab)

		const router = options.router
		const location = getTabLocation(index, tab)
		if (router && location !== undefined) {
			try {
				return router.resolve(location).fullPath === router.currentRoute.value.fullPath
			} catch {
				return false
			}
		}

		return index === actualActiveIndex.value
	}

	async function applyPendingTabChange(pending: PendingTab<Tab>) {
		if (options.changeTab) {
			return (await options.changeTab(pending.index, pending.tab)) !== false
		}

		const router = options.router
		const location = getTabLocation(pending.index, pending.tab)
		if (!router || location === undefined) return false

		const failure = await (options.replace ? router.replace(location) : router.push(location))
		return !failure
	}

	function resetPendingTab() {
		pendingTab.value = null
		optimisticActiveIndex.value = null
		applyingPendingTab.value = false
		pendingChangeApplied.value = false
		leaveFinished.value = false
		visible.value = true
	}

	async function revealPendingTabContent(pendingToken: number) {
		if (
			pendingTab.value?.token !== pendingToken ||
			!pendingChangeApplied.value ||
			!leaveFinished.value
		) {
			return
		}

		await nextTick()

		if (
			pendingTab.value?.token !== pendingToken ||
			!pendingChangeApplied.value ||
			!leaveFinished.value
		) {
			return
		}

		visible.value = true
	}

	async function runPendingTabChange() {
		const pending = pendingTab.value
		if (!pending || applyingPendingTab.value) return

		applyingPendingTab.value = true

		let changed = false
		try {
			changed = await applyPendingTabChange(pending)
		} catch {
			changed = false
		}

		if (pendingTab.value?.token !== pending.token) return

		if (!changed) {
			resetPendingTab()
			return
		}

		await nextTick()

		if (pendingTab.value?.token !== pending.token) return

		pendingChangeApplied.value = true
		void revealPendingTabContent(pending.token)
	}

	function selectTab(index: number, tab: Tab) {
		tab.onHover?.()

		if (pendingTab.value || applyingPendingTab.value) return
		if (isCurrentTab(index, tab) && visible.value) return

		const currentIndex = activeIndex.value
		if (isValidIndex(currentIndex) && isValidIndex(index) && currentIndex !== index) {
			direction.value = getDirection(currentIndex, index)
		}

		pendingTab.value = { index, tab, token: ++token }
		pendingChangeApplied.value = false
		leaveFinished.value = false
		optimisticActiveIndex.value = index
		visible.value = false
	}

	function handleBeforeLeave() {
		return runPendingTabChange()
	}

	function handleAfterLeave() {
		const pending = pendingTab.value
		if (!pending) return

		leaveFinished.value = true
		void revealPendingTabContent(pending.token)
	}

	function finishTransition() {
		resetPendingTab()
	}

	watch(actualActiveIndex, (next, previous) => {
		if (pendingTab.value) return
		if (!isValidIndex(next) || !isValidIndex(previous) || next === previous) return

		direction.value = getDirection(previous, next)
	})

	return {
		activeIndex,
		direction,
		visible,
		selectTab,
		handleBeforeLeave,
		handleAfterLeave,
		handleAfterEnter: finishTransition,
		handleEnterCancelled: finishTransition,
		handleLeaveCancelled: finishTransition,
	}
}
