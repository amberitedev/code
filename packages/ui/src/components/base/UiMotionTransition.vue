<template>
	<div ref="host" class="ui-motion-host" :style="hostStyle">
		<Transition
			name="ui-motion"
			:mode="transitionMode"
			:duration="transitionDuration"
			@before-enter="startEnterTransition"
			@before-leave="startLeaveTransition"
			@after-enter="finishEnterTransition"
			@after-leave="finishLeaveTransition"
			@enter-cancelled="cancelEnterTransition"
			@leave-cancelled="cancelLeaveTransition"
		>
			<div
				v-if="visible"
				:key="contentKey"
				class="ui-motion-frame"
				:class="{ 'ui-motion-frame--covered': frozenLeaveActive }"
				:style="motionStyle"
			>
				<slot />
			</div>
		</Transition>
	</div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'

import {
	getUiMotionEnterTransform,
	getUiMotionLeaveTransform,
	installUiMotionFreezeFrameDebugControl,
	isUiMotionFreezeFrameHiddenForDebug,
	normalizeUiMotionDirection,
	resolveUiMotionConfig,
	type UiMotionConfig,
	type UiMotionDirection,
	type UiMotionEasingPreset,
	type UiMotionMode,
	type UiMotionType,
	UI_MOTION_FREEZE_FRAME_DEBUG_EVENT,
	UI_MOTION_PRESETS,
} from '#ui/composables/ui-motion'

const props = withDefaults(
	defineProps<{
		contentKey?: string | number
		config?: UiMotionConfig
		enabled?: boolean
		type?: UiMotionType
		direction?: UiMotionDirection
		enterMs?: number
		leaveMs?: number
		easing?: UiMotionEasingPreset | string
		enterEasing?: string
		leaveEasing?: string
		distance?: string
		scale?: number
		mode?: UiMotionMode
		lockHeight?: boolean
		freezeLeave?: boolean
		safetyMs?: number
		visible?: boolean
	}>(),
	{
		contentKey: '',
		config: undefined,
		enabled: undefined,
		type: undefined,
		direction: undefined,
		enterMs: undefined,
		leaveMs: undefined,
		easing: undefined,
		enterEasing: undefined,
		leaveEasing: undefined,
		distance: undefined,
		scale: undefined,
		mode: undefined,
		lockHeight: undefined,
		freezeLeave: undefined,
		safetyMs: undefined,
		visible: true,
	},
)

const emit = defineEmits<{
	(e: 'before-enter', el: Element): void
	(e: 'before-leave', el: Element): void
	(e: 'after-enter', el: Element): void
	(e: 'after-leave', el: Element): void
	(e: 'enter-cancelled', el: Element): void
	(e: 'leave-cancelled', el: Element): void
}>()

const motion = computed(() =>
	resolveUiMotionConfig(
		{
			...props.config,
			enabled: props.enabled ?? props.config?.enabled,
			type: props.type ?? props.config?.type,
			direction: props.direction ?? props.config?.direction,
			enterMs: props.enterMs ?? props.config?.enterMs,
			leaveMs: props.leaveMs ?? props.config?.leaveMs,
			easing: props.easing ?? props.config?.easing,
			enterEasing: props.enterEasing ?? props.config?.enterEasing,
			leaveEasing: props.leaveEasing ?? props.config?.leaveEasing,
			distance: props.distance ?? props.config?.distance,
			scale: props.scale ?? props.config?.scale,
			mode: props.mode ?? props.config?.mode,
			lockHeight: props.lockHeight ?? props.config?.lockHeight,
			freezeLeave: props.freezeLeave ?? props.config?.freezeLeave,
			safetyMs: props.safetyMs ?? props.config?.safetyMs,
		},
		UI_MOTION_PRESETS.contentSlide,
	),
)
const transitionMode = computed(() => {
	if (!motion.value.enabled || motion.value.type === 'none' || motion.value.mode === 'default') {
		return undefined
	}

	return motion.value.mode
})
const transitionDuration = computed(() => ({
	enter: motion.value.enabled ? motion.value.enterMs : 0,
	leave: motion.value.enabled ? motion.value.leaveMs : 0,
}))
const motionStyle = computed(() => {
	const enabled = motion.value.enabled && motion.value.type !== 'none'
	const fadesFrame =
		enabled &&
		(motion.value.type === 'slide' ||
			motion.value.type === 'fade' ||
			motion.value.type === 'scaleFade')
	const enterOpacity = fadesFrame ? 0 : 1
	const leaveOpacity = fadesFrame ? 0 : 1

	return {
		'--ui-motion-enter-ms': `${transitionDuration.value.enter}ms`,
		'--ui-motion-leave-ms': `${transitionDuration.value.leave}ms`,
		'--ui-motion-enter-easing': motion.value.enterEasing,
		'--ui-motion-leave-easing': motion.value.leaveEasing,
		'--ui-motion-enter-opacity': `${enterOpacity}`,
		'--ui-motion-leave-opacity': `${leaveOpacity}`,
		'--ui-motion-enter-transform': enabled ? getUiMotionEnterTransform(motion.value) : 'none',
		'--ui-motion-leave-transform': enabled ? getUiMotionLeaveTransform(motion.value) : 'none',
	}
})
const host = ref<HTMLElement | null>(null)
const lockedHeight = ref<string | undefined>()
const heightTransitionActive = ref(false)
const frozenLeaveActive = ref(false)
const hostStyle = computed(() => ({
	minHeight: lockedHeight.value,
	overflow: lockedHeight.value && motion.value.type === 'height' ? 'hidden' : undefined,
	transition: heightTransitionActive.value
		? `min-height ${transitionDuration.value.enter}ms ${motion.value.enterEasing}`
		: undefined,
}))

let safetyTimer: ReturnType<typeof setTimeout> | undefined
let heightReleaseTimer: ReturnType<typeof setTimeout> | undefined
let heightAnimationFrame: number | undefined
let frozenLeaveCoverElement: HTMLElement | undefined
let frozenLeaveElement: HTMLElement | undefined
let frozenLeaveAnimation: Animation | undefined
let frozenLeaveTimer: ReturnType<typeof setTimeout> | undefined
let hiddenLeaveElement: HTMLElement | undefined
let hiddenLeaveElementVisibility = ''

function clearSafetyTimer() {
	if (!safetyTimer) return

	clearTimeout(safetyTimer)
	safetyTimer = undefined
}

function clearHeightReleaseTimer() {
	if (!heightReleaseTimer) return

	clearTimeout(heightReleaseTimer)
	heightReleaseTimer = undefined
}

function clearHeightAnimationFrame() {
	if (heightAnimationFrame === undefined) return

	cancelAnimationFrame(heightAnimationFrame)
	heightAnimationFrame = undefined
}

function clearFrozenLeaveElement() {
	if (frozenLeaveTimer) {
		clearTimeout(frozenLeaveTimer)
		frozenLeaveTimer = undefined
	}

	frozenLeaveAnimation?.cancel()
	frozenLeaveAnimation = undefined

	frozenLeaveCoverElement?.remove()
	frozenLeaveCoverElement = undefined

	frozenLeaveElement?.remove()
	frozenLeaveElement = undefined
	frozenLeaveActive.value = false

	if (hiddenLeaveElement) {
		hiddenLeaveElement.style.visibility = hiddenLeaveElementVisibility
		hiddenLeaveElement = undefined
		hiddenLeaveElementVisibility = ''
	}
}

function lockHostHeight(el?: Element) {
	if (!motion.value.lockHeight && motion.value.type !== 'height') return

	const height = el instanceof HTMLElement ? el.offsetHeight : (host.value?.offsetHeight ?? 0)
	if (height > 0) {
		lockedHeight.value = `${height}px`
	}
}

function releaseHostHeight() {
	clearHeightReleaseTimer()
	clearHeightAnimationFrame()
	heightTransitionActive.value = false
	lockedHeight.value = undefined
}

function scheduleHostHeightRelease() {
	clearHeightReleaseTimer()
	heightReleaseTimer = setTimeout(() => {
		heightReleaseTimer = undefined
		releaseHostHeight()
	}, motion.value.safetyMs)
}

function startHeightTransition(el?: Element) {
	if (motion.value.type !== 'height') return

	const targetHeight = el instanceof HTMLElement ? el.offsetHeight : 0
	if (targetHeight <= 0) return

	clearHeightAnimationFrame()
	lockedHeight.value = lockedHeight.value ?? `${host.value?.offsetHeight ?? targetHeight}px`
	heightTransitionActive.value = true

	if (typeof window === 'undefined') {
		lockedHeight.value = `${targetHeight}px`
		return
	}

	heightAnimationFrame = requestAnimationFrame(() => {
		heightAnimationFrame = undefined
		lockedHeight.value = `${targetHeight}px`
	})
}

function finishTransition(el?: Element) {
	clearSafetyTimer()
	el?.getAnimations?.().forEach((animation) => {
		if (animation.playState !== 'finished') {
			animation.cancel()
		}
	})
}

function getFrameFades() {
	return (
		motion.value.enabled &&
		(motion.value.type === 'slide' ||
			motion.value.type === 'fade' ||
			motion.value.type === 'scaleFade')
	)
}

function getFreezeCoverBleed() {
	if (motion.value.type !== 'slide') {
		return { x: '0px', y: '0px' }
	}

	switch (normalizeUiMotionDirection(motion.value.direction)) {
		case 'left':
		case 'right':
			return { x: motion.value.distance, y: '0px' }
		case 'up':
		case 'down':
			return { x: '0px', y: motion.value.distance }
		default:
			return { x: '0px', y: '0px' }
	}
}

function handleFreezeFrameDebugChange() {
	if (isUiMotionFreezeFrameHiddenForDebug()) {
		clearFrozenLeaveElement()
	}
}

function freezeLeaveElement(el?: Element) {
	if (
		!motion.value.freezeLeave ||
		isUiMotionFreezeFrameHiddenForDebug() ||
		!motion.value.enabled ||
		motion.value.type === 'none' ||
		!(el instanceof HTMLElement) ||
		!host.value ||
		typeof window === 'undefined'
	) {
		return
	}

	clearFrozenLeaveElement()

	const hostRect = host.value.getBoundingClientRect()
	const elementRect = el.getBoundingClientRect()
	const cover = document.createElement('div')
	const clone = el.cloneNode(true) as HTMLElement
	const frozenFrameBounds = {
		left: `${elementRect.left - hostRect.left}px`,
		top: `${elementRect.top - hostRect.top}px`,
		width: `${elementRect.width}px`,
		height: `${elementRect.height}px`,
	}
	const coverBleed = getFreezeCoverBleed()
	const frozenCoverBounds = {
		left: `calc(${frozenFrameBounds.left} - ${coverBleed.x})`,
		top: `calc(${frozenFrameBounds.top} - ${coverBleed.y})`,
		width: `calc(${frozenFrameBounds.width} + ${coverBleed.x} + ${coverBleed.x})`,
		height: `calc(${frozenFrameBounds.height} + ${coverBleed.y} + ${coverBleed.y})`,
	}

	cover.setAttribute('aria-hidden', 'true')
	cover.classList.add('ui-motion-freeze-cover')
	Object.assign(cover.style, frozenCoverBounds)

	clone.setAttribute('aria-hidden', 'true')
	clone.setAttribute('inert', '')
	clone.classList.add('ui-motion-frozen-leave-frame')
	Object.assign(clone.style, {
		position: 'absolute',
		...frozenFrameBounds,
		margin: '0',
		pointerEvents: 'none',
		transform: 'none',
		transformOrigin: 'top left',
		opacity: '1',
		zIndex: '10001',
	})

	frozenLeaveActive.value = true
	hiddenLeaveElement = el
	hiddenLeaveElementVisibility = el.style.visibility
	el.style.visibility = 'hidden'

	host.value.appendChild(cover)
	host.value.appendChild(clone)
	frozenLeaveCoverElement = cover
	frozenLeaveElement = clone

	const leaveOpacity = getFrameFades() ? 0 : 1
	const leaveTransform = getUiMotionLeaveTransform(motion.value)
	const leaveMs = transitionDuration.value.leave

	if (typeof clone.animate === 'function' && leaveMs > 0) {
		frozenLeaveAnimation = clone.animate(
			[
				{ opacity: 1, transform: 'none' },
				{ opacity: leaveOpacity, transform: leaveTransform },
			],
			{
				duration: leaveMs,
				easing: motion.value.leaveEasing,
				fill: 'forwards',
			},
		)
		return
	}

	clone.style.opacity = `${leaveOpacity}`
	clone.style.transform = leaveTransform
	frozenLeaveTimer = setTimeout(() => {
		frozenLeaveTimer = undefined
	}, leaveMs)
}

function startTransition(el?: Element) {
	clearSafetyTimer()
	safetyTimer = setTimeout(() => {
		finishTransition(el)
	}, motion.value.safetyMs)
}

function startLeaveTransition(el?: Element) {
	clearHeightReleaseTimer()
	lockHostHeight(el)
	freezeLeaveElement(el)
	startTransition(el)
	if (el) emit('before-leave', el)
}

function startEnterTransition(el?: Element) {
	clearHeightReleaseTimer()
	startHeightTransition(el)
	startTransition(el)
	if (el) emit('before-enter', el)
}

function finishLeaveTransition(el?: Element) {
	finishTransition(el)
	clearFrozenLeaveElement()
	scheduleHostHeightRelease()
	if (el) emit('after-leave', el)
}

function finishEnterTransition(el?: Element) {
	finishTransition(el)
	releaseHostHeight()
	if (el) emit('after-enter', el)
}

function cancelLeaveTransition(el?: Element) {
	finishTransition(el)
	clearFrozenLeaveElement()
	scheduleHostHeightRelease()
	if (el) emit('leave-cancelled', el)
}

function cancelEnterTransition(el?: Element) {
	finishTransition(el)
	releaseHostHeight()
	if (el) emit('enter-cancelled', el)
}

installUiMotionFreezeFrameDebugControl()

onMounted(() => {
	if (typeof window !== 'undefined') {
		window.addEventListener(UI_MOTION_FREEZE_FRAME_DEBUG_EVENT, handleFreezeFrameDebugChange)
	}
})

onUnmounted(() => {
	if (typeof window !== 'undefined') {
		window.removeEventListener(UI_MOTION_FREEZE_FRAME_DEBUG_EVENT, handleFreezeFrameDebugChange)
	}

	clearSafetyTimer()
	clearHeightReleaseTimer()
	clearHeightAnimationFrame()
	clearFrozenLeaveElement()
})
</script>

<style>
.ui-motion-host {
	position: relative;
	isolation: isolate;
	width: 100%;
	height: 100%;
	min-height: 100%;
	min-width: 0;
}

.ui-motion-frame {
	position: relative;
	width: 100%;
	height: 100%;
	min-height: inherit;
	min-width: 0;
}

.ui-motion-frame--covered {
	visibility: hidden;
}

.ui-motion-freeze-cover {
	position: absolute;
	background: var(--color-bg);
	pointer-events: none;
	z-index: 10000;
}

.ui-motion-frozen-leave-frame {
	will-change: opacity, transform;
}

.ui-motion-enter-active {
	transition:
		opacity var(--ui-motion-enter-ms) var(--ui-motion-enter-easing),
		transform var(--ui-motion-enter-ms) var(--ui-motion-enter-easing);
	will-change: opacity, transform;
}

.ui-motion-leave-active {
	transition:
		opacity var(--ui-motion-leave-ms) var(--ui-motion-leave-easing),
		transform var(--ui-motion-leave-ms) var(--ui-motion-leave-easing);
	will-change: opacity, transform;
}

.ui-motion-enter-from {
	opacity: var(--ui-motion-enter-opacity);
	transform: var(--ui-motion-enter-transform);
}

.ui-motion-leave-to {
	opacity: var(--ui-motion-leave-opacity);
	transform: var(--ui-motion-leave-transform);
}

@media (prefers-reduced-motion: reduce) {
	.ui-motion-enter-active,
	.ui-motion-leave-active {
		transition: none;
	}
}
</style>
