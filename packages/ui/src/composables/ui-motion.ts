export type UiMotionDirection = 'forward' | 'backward' | 'left' | 'right' | 'up' | 'down' | 'middle'
export type UiMotionType = 'slide' | 'fade' | 'scaleFade' | 'height' | 'none'
export type UiMotionMode = 'out-in' | 'in-out' | 'default'
export type UiMotionEasingPreset = 'content-slide' | 'snappy' | 'standard' | 'linear'

export interface UiMotionConfig {
	enabled?: boolean
	type?: UiMotionType
	direction?: UiMotionDirection
	enterMs?: number
	leaveMs?: number
	enterEasing?: string
	leaveEasing?: string
	easing?: UiMotionEasingPreset | string
	distance?: string
	scale?: number
	mode?: UiMotionMode
	lockHeight?: boolean
	freezeLeave?: boolean
	safetyMs?: number
}

export interface ResolvedUiMotionConfig {
	enabled: boolean
	type: UiMotionType
	direction: UiMotionDirection
	enterMs: number
	leaveMs: number
	enterEasing: string
	leaveEasing: string
	distance: string
	scale: number
	mode: UiMotionMode
	lockHeight: boolean
	freezeLeave: boolean
	safetyMs: number
}

export const UI_MOTION_CONTENT_MS = 120
export const UI_MOTION_ROUTE_MS = 160
export const UI_MOTION_TAB_MS = 120
export const UI_MOTION_HEIGHT_MS = 140
export const UI_MOTION_SAFETY_BUFFER_MS = 100
export const UI_MOTION_NAV_TABS_SLIDER_MS = 150
export const UI_MOTION_NAV_TABS_SLIDER_STAGGER_DELAY_MS = 200
export const UI_MOTION_NAV_TABS_SLIDER_EASING = 'cubic-bezier(0.4, 0, 0.2, 1)'
export const UI_MOTION_NAV_TABS_FADE_MS = 250
export const UI_MOTION_NAV_TABS_FADE_EASING = 'cubic-bezier(0.5, 0, 0.2, 1)'
export const UI_MOTION_NAV_TABS_FADE_DELAY_MS = 50
export const UI_MOTION_FREEZE_FRAME_HIDDEN_STORAGE_KEY = 'amberite:ui-motion-hide-freeze-frame'
export const UI_MOTION_FREEZE_FRAME_DEBUG_EVENT = 'ui-motion-freeze-frame-debug'
export const UI_GHOST_SHIMMER_MS = 3200

declare global {
	interface Window {
		__setUiMotionFreezeFrameHidden?: (hidden?: boolean) => boolean
	}
}

let uiMotionFreezeFrameHiddenForDebug = false

export const UI_MOTION_DIRECTIONS: UiMotionDirection[] = [
	'forward',
	'backward',
	'right',
	'left',
	'up',
	'down',
	'middle',
]

export const UI_MOTION_EASINGS: Record<UiMotionEasingPreset, { enter: string; leave: string }> = {
	'content-slide': {
		enter: 'cubic-bezier(0, 0.55, 0.45, 1)',
		leave: 'cubic-bezier(0.55, 0, 1, 0.45)',
	},
	snappy: {
		enter: 'cubic-bezier(0.2, 0.8, 0.2, 1)',
		leave: 'cubic-bezier(0.4, 0, 1, 1)',
	},
	standard: {
		enter: 'cubic-bezier(0.4, 0, 0.2, 1)',
		leave: 'cubic-bezier(0.4, 0, 0.2, 1)',
	},
	linear: {
		enter: 'linear',
		leave: 'linear',
	},
}

export const UI_MOTION_PRESETS = {
	contentSlide: {
		enabled: true,
		type: 'slide',
		direction: 'forward',
		enterMs: UI_MOTION_CONTENT_MS,
		leaveMs: UI_MOTION_CONTENT_MS,
		easing: 'content-slide',
		distance: '1rem',
		mode: 'out-in',
		lockHeight: true,
		freezeLeave: true,
	} satisfies UiMotionConfig,
	routeSlide: {
		enabled: true,
		type: 'slide',
		direction: 'forward',
		enterMs: UI_MOTION_ROUTE_MS,
		leaveMs: UI_MOTION_ROUTE_MS,
		easing: 'content-slide',
		distance: '1.25rem',
		mode: 'out-in',
		lockHeight: true,
		freezeLeave: true,
	} satisfies UiMotionConfig,
	tabSlide: {
		enabled: true,
		type: 'slide',
		direction: 'forward',
		enterMs: UI_MOTION_TAB_MS,
		leaveMs: UI_MOTION_TAB_MS,
		easing: 'content-slide',
		distance: '1rem',
		mode: 'out-in',
		lockHeight: true,
		freezeLeave: true,
	} satisfies UiMotionConfig,
	fade: {
		enabled: true,
		type: 'fade',
		direction: 'middle',
		enterMs: UI_MOTION_CONTENT_MS,
		leaveMs: UI_MOTION_CONTENT_MS,
		easing: 'content-slide',
		distance: '0',
		mode: 'out-in',
		lockHeight: true,
		freezeLeave: true,
	} satisfies UiMotionConfig,
	scaleFade: {
		enabled: true,
		type: 'scaleFade',
		direction: 'middle',
		enterMs: UI_MOTION_CONTENT_MS,
		leaveMs: UI_MOTION_CONTENT_MS,
		easing: 'standard',
		distance: '0',
		scale: 0.98,
		mode: 'out-in',
		lockHeight: true,
		freezeLeave: true,
	} satisfies UiMotionConfig,
	height: {
		enabled: true,
		type: 'height',
		direction: 'middle',
		enterMs: UI_MOTION_HEIGHT_MS,
		leaveMs: UI_MOTION_HEIGHT_MS,
		easing: 'standard',
		distance: '0',
		mode: 'out-in',
		lockHeight: true,
	} satisfies UiMotionConfig,
	none: {
		enabled: false,
		type: 'none',
		direction: 'middle',
		enterMs: 0,
		leaveMs: 0,
		easing: 'linear',
		distance: '0',
		mode: 'default',
		lockHeight: false,
	} satisfies UiMotionConfig,
}

function firstDefined<T>(...values: (T | undefined)[]): T | undefined {
	return values.find((value) => value !== undefined)
}

function prefersReducedMotion() {
	return (
		typeof window !== 'undefined' &&
		typeof window.matchMedia === 'function' &&
		window.matchMedia('(prefers-reduced-motion: reduce)').matches
	)
}

function getUiMotionDebugStorage() {
	if (typeof window === 'undefined') return undefined

	try {
		return window.localStorage
	} catch {
		return undefined
	}
}

export function isUiMotionFreezeFrameHiddenForDebug() {
	return (
		uiMotionFreezeFrameHiddenForDebug ||
		getUiMotionDebugStorage()?.getItem(UI_MOTION_FREEZE_FRAME_HIDDEN_STORAGE_KEY) === 'true'
	)
}

export function setUiMotionFreezeFrameHiddenForDebug(hidden = true) {
	const storage = getUiMotionDebugStorage()
	uiMotionFreezeFrameHiddenForDebug = hidden

	try {
		if (hidden) {
			storage?.setItem(UI_MOTION_FREEZE_FRAME_HIDDEN_STORAGE_KEY, 'true')
		} else {
			storage?.removeItem(UI_MOTION_FREEZE_FRAME_HIDDEN_STORAGE_KEY)
		}
	} catch {
		// Debug-only toggle; ignore unavailable storage.
	}

	if (typeof window !== 'undefined') {
		window.dispatchEvent(
			new CustomEvent(UI_MOTION_FREEZE_FRAME_DEBUG_EVENT, {
				detail: { hidden },
			}),
		)
	}

	return hidden
}

export function installUiMotionFreezeFrameDebugControl() {
	if (typeof window === 'undefined') return

	window.__setUiMotionFreezeFrameHidden = setUiMotionFreezeFrameHiddenForDebug
}

export function getUiMotionEasing(easing?: UiMotionEasingPreset | string) {
	if (!easing) return UI_MOTION_EASINGS['content-slide']
	if (Object.prototype.hasOwnProperty.call(UI_MOTION_EASINGS, easing)) {
		return UI_MOTION_EASINGS[easing as UiMotionEasingPreset]
	}

	return {
		enter: easing,
		leave: easing,
	}
}

export function normalizeUiMotionDirection(direction: UiMotionDirection): UiMotionDirection {
	if (direction === 'forward') return 'right'
	if (direction === 'backward') return 'left'

	return direction
}

export function reverseUiMotionDirection(direction: UiMotionDirection): UiMotionDirection {
	switch (normalizeUiMotionDirection(direction)) {
		case 'left':
			return 'right'
		case 'right':
			return 'left'
		case 'up':
			return 'down'
		case 'down':
			return 'up'
		default:
			return 'middle'
	}
}

export function resolveUiMotionConfig(
	config?: UiMotionConfig,
	fallback: UiMotionConfig = UI_MOTION_PRESETS.contentSlide,
): ResolvedUiMotionConfig {
	const easing = getUiMotionEasing(firstDefined(config?.easing, fallback.easing))
	const enterMs = firstDefined(
		config?.enterMs,
		fallback.enterMs,
		UI_MOTION_PRESETS.contentSlide.enterMs,
	)
	const leaveMs = firstDefined(config?.leaveMs, fallback.leaveMs, enterMs)
	const safetyMs = firstDefined(
		config?.safetyMs,
		fallback.safetyMs,
		Math.max(enterMs, leaveMs) + UI_MOTION_SAFETY_BUFFER_MS,
	)
	const type = firstDefined(config?.type, fallback.type, 'slide')
	const freezesByDefault = type === 'slide' || type === 'fade' || type === 'scaleFade'
	const reducedMotion = prefersReducedMotion()
	const keepReducedFade =
		reducedMotion && type === 'fade' && Math.max(enterMs, leaveMs) <= UI_MOTION_CONTENT_MS
	const configuredEnabled = firstDefined(config?.enabled, fallback.enabled, true)
	const enabled = reducedMotion ? configuredEnabled && keepReducedFade : configuredEnabled

	return {
		enabled,
		type: reducedMotion && !keepReducedFade ? 'none' : type,
		direction: firstDefined(config?.direction, fallback.direction, 'forward'),
		enterMs: reducedMotion && !keepReducedFade ? 0 : enterMs,
		leaveMs: reducedMotion && !keepReducedFade ? 0 : leaveMs,
		enterEasing: firstDefined(config?.enterEasing, fallback.enterEasing, easing.enter),
		leaveEasing: firstDefined(config?.leaveEasing, fallback.leaveEasing, easing.leave),
		distance: firstDefined(config?.distance, fallback.distance, '1rem'),
		scale: firstDefined(config?.scale, fallback.scale, 0.98),
		mode: firstDefined(config?.mode, fallback.mode, 'out-in'),
		lockHeight: firstDefined(config?.lockHeight, fallback.lockHeight, true),
		freezeLeave: firstDefined(config?.freezeLeave, fallback.freezeLeave, freezesByDefault),
		safetyMs: reducedMotion && !keepReducedFade ? 0 : safetyMs,
	}
}

function negativeDistance(distance: string) {
	return `calc(${distance} * -1)`
}

export function getUiMotionEnterTransform(config: ResolvedUiMotionConfig) {
	if (config.type === 'scaleFade') return `scale(${config.scale})`
	if (config.type !== 'slide') return 'none'

	switch (normalizeUiMotionDirection(config.direction)) {
		case 'left':
			return `translate3d(${negativeDistance(config.distance)}, 0, 0)`
		case 'right':
			return `translate3d(${config.distance}, 0, 0)`
		case 'up':
			return `translate3d(0, ${negativeDistance(config.distance)}, 0)`
		case 'down':
			return `translate3d(0, ${config.distance}, 0)`
		default:
			return 'none'
	}
}

export function getUiMotionLeaveTransform(config: ResolvedUiMotionConfig) {
	if (config.type === 'scaleFade') return `scale(${config.scale})`
	if (config.type !== 'slide') return 'none'

	switch (normalizeUiMotionDirection(config.direction)) {
		case 'left':
			return `translate3d(${config.distance}, 0, 0)`
		case 'right':
			return `translate3d(${negativeDistance(config.distance)}, 0, 0)`
		case 'up':
			return `translate3d(0, ${config.distance}, 0)`
		case 'down':
			return `translate3d(0, ${negativeDistance(config.distance)}, 0)`
		default:
			return 'none'
	}
}

export function getUiMotionTypeForDirection(direction: UiMotionDirection): UiMotionType {
	return normalizeUiMotionDirection(direction) === 'middle' ? 'fade' : 'slide'
}
