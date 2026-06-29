import { useStorage } from '@vueuse/core'
import { computed } from 'vue'

import type { UiMotionDirection } from './ui-motion'
import { UI_MOTION_EASINGS, UI_MOTION_SAFETY_BUFFER_MS, UI_MOTION_TAB_MS } from './ui-motion'

export type NavTabContentDirection = UiMotionDirection

export const NAV_TAB_CONTENT_TRANSITION_MS = UI_MOTION_TAB_MS
export const NAV_TAB_CONTENT_TRANSITION_SAFETY_MS =
	NAV_TAB_CONTENT_TRANSITION_MS + UI_MOTION_SAFETY_BUFFER_MS
export const NAV_TAB_CONTENT_LEAVE_EASING = UI_MOTION_EASINGS['content-slide'].leave
export const NAV_TAB_CONTENT_ENTER_EASING = UI_MOTION_EASINGS['content-slide'].enter
export const NAV_TAB_CONTENT_SLOW_MOTION_STORAGE_KEY = 'amberite:page-slide-slow-motion'
export const NAV_TAB_CONTENT_SLOW_MOTION_MULTIPLIER = 5

export function getNavTabContentTransitionName(direction: NavTabContentDirection) {
	return `ui-motion-${direction}`
}

function createNavTabContentTransitionTiming() {
	const slowMotion = useStorage(NAV_TAB_CONTENT_SLOW_MOTION_STORAGE_KEY, false)
	const multiplier = computed(() =>
		slowMotion.value ? NAV_TAB_CONTENT_SLOW_MOTION_MULTIPLIER : 1,
	)
	const durationMs = computed(() => NAV_TAB_CONTENT_TRANSITION_MS * multiplier.value)
	const durationCss = computed(() => `${durationMs.value}ms`)
	const safetyMs = computed(() => NAV_TAB_CONTENT_TRANSITION_SAFETY_MS * multiplier.value)

	if (typeof window !== 'undefined') {
		Object.defineProperty(window, '__setPageSlideSlowMotion', {
			configurable: true,
			value: (enabled = true) => {
				slowMotion.value = !!enabled
			},
		})
	}

	return {
		durationMs,
		durationCss,
		enterEasing: NAV_TAB_CONTENT_ENTER_EASING,
		leaveEasing: NAV_TAB_CONTENT_LEAVE_EASING,
		safetyMs,
		slowMotion,
	}
}

let timingState: ReturnType<typeof createNavTabContentTransitionTiming> | undefined

export function useNavTabContentTransitionTiming() {
	if (timingState) return timingState

	timingState = createNavTabContentTransitionTiming()

	return timingState
}
