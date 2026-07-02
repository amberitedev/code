<!--
App freeze-frame transition summary:
- runLeaveTransition animates the current DOM out before route/state changes.
- runEnterTransition reveals and animates the real incoming content after it is mounted.
- Freeze-frame slots remain supported for callers that provide snapshots, but browse tab switches
	now leave with the live DOM to avoid synchronous capture jank.
-->
<template>
	<div ref="host" class="app-freeze-frame-transition" :style="{ minHeight: lockedHeight }">
		<div
			v-if="renderContent"
			ref="contentFrameElement"
			:key="contentKey"
			class="app-freeze-frame-transition__frame"
			:class="{ 'app-freeze-frame-transition__frame--covered': freezeFrameVisible }"
		>
			<slot />
		</div>

		<Teleport to="body">
			<div
				v-if="freezeFrameVisible"
				class="app-freeze-frame-transition__freeze-cover"
				aria-hidden="true"
				inert
			>
				<div
					v-if="outgoingFreezeSnapshot !== null"
					ref="outgoingFreezeFrameElement"
					class="app-freeze-frame-transition__freeze-frame app-freeze-frame-transition__freeze-frame--outgoing"
				>
					<slot
						name="freeze-frame"
						:snapshot="outgoingFreezeSnapshot"
						phase="outgoing"
						:direction="motion.direction"
						:transitioning="transitioningActive"
					>
						<component
							:is="freezeFrame"
							v-if="freezeFrame"
							:snapshot="outgoingFreezeSnapshot"
							phase="outgoing"
							:direction="motion.direction"
							:transitioning="transitioningActive"
						/>
					</slot>
				</div>
			</div>
		</Teleport>
	</div>
</template>

<script setup lang="ts">
import type { Component } from 'vue'
import { computed, nextTick, onUnmounted, ref, shallowRef, watch } from 'vue'
import {
	getUiMotionEnterTransform,
	getUiMotionLeaveTransform,
	isUiMotionFreezeFrameHiddenForDebug,
	resolveUiMotionConfig,
	type UiMotionDirection,
	UI_MOTION_PRESETS,
	UI_MOTION_SAFETY_BUFFER_MS,
	useNavTabContentTransitionTiming,
} from '@modrinth/ui'

const props = withDefaults(
	defineProps<{
		contentKey?: string | number
		direction?: UiMotionDirection
		visible?: boolean
		freezeFrame?: Component
		createFreezeFrame?: () => unknown | null | undefined
		prepareIncomingFreezeFrame?: (el?: Element) => void | Promise<void>
		isIncomingReady?: () => boolean
		incomingHoldMs?: number
		heightReleaseDelayMs?: number
	}>(),
	{
		contentKey: '',
		visible: true,
		incomingHoldMs: 160,
		heightReleaseDelayMs: 0,
	},
)

const emit = defineEmits<{
	(e: 'update:transitioning', transitioning: boolean): void
	(e: 'before-enter', el: Element): void
	(e: 'before-leave', el: Element): void
	(e: 'after-enter', el: Element): void
	(e: 'after-leave', el: Element): void
	(e: 'enter-cancelled', el: Element): void
	(e: 'leave-cancelled', el: Element): void
}>()

const timing = useNavTabContentTransitionTiming()
const transitionMs = computed(() => timing.durationMs.value)
const motion = computed(() =>
	resolveUiMotionConfig(
		{
			...UI_MOTION_PRESETS.tabSlide,
			direction: props.direction ?? UI_MOTION_PRESETS.tabSlide.direction,
			enterMs: transitionMs.value,
			leaveMs: transitionMs.value,
			enterEasing: timing.enterEasing,
			leaveEasing: timing.leaveEasing,
			safetyMs: transitionMs.value + UI_MOTION_SAFETY_BUFFER_MS,
		},
		UI_MOTION_PRESETS.tabSlide,
	),
)
const transitionDuration = computed(() => ({
	enter: motion.value.enabled ? motion.value.enterMs : 0,
	leave: motion.value.enabled ? motion.value.leaveMs : 0,
}))
const fadesFrame = computed(
	() =>
		motion.value.enabled &&
		(motion.value.type === 'slide' ||
			motion.value.type === 'fade' ||
			motion.value.type === 'scaleFade'),
)
const host = ref<HTMLElement | null>(null)
const contentFrameElement = ref<HTMLElement | null>(null)
const outgoingFreezeFrameElement = ref<HTMLElement | null>(null)
const lockedHeight = ref<string | undefined>()
const outgoingFreezeSnapshot = shallowRef<unknown | null>(null)
const renderContent = ref(props.visible)
const freezeFrameVisible = ref(false)
const transitioningActive = ref(false)

let heightReleaseTimer: ReturnType<typeof setTimeout> | undefined
let safetyTimer: ReturnType<typeof setTimeout> | undefined
let outgoingFreezeFrameAnimation: Animation | undefined
let contentFrameAnimation: Animation | undefined
let transitionToken = 0

function clearHeightReleaseTimer() {
	if (!heightReleaseTimer) return

	clearTimeout(heightReleaseTimer)
	heightReleaseTimer = undefined
}

function clearSafetyTimer() {
	if (!safetyTimer) return

	clearTimeout(safetyTimer)
	safetyTimer = undefined
}

function cancelContentFrameAnimation() {
	contentFrameAnimation?.cancel()
	contentFrameAnimation = undefined
	resetContentFrameStyle(contentFrameElement.value)
}

function clearFreezeFrame() {
	if (outgoingFreezeFrameElement.value) {
		outgoingFreezeFrameElement.value.style.visibility = 'hidden'
	}
	outgoingFreezeFrameAnimation?.cancel()
	outgoingFreezeFrameAnimation = undefined
	freezeFrameVisible.value = false
	outgoingFreezeSnapshot.value = null
}

function setTransitioning(transitioning: boolean) {
	if (transitioningActive.value === transitioning) return

	transitioningActive.value = transitioning
	emit('update:transitioning', transitioning)
}

function lockHostHeight(el?: Element) {
	if (!motion.value.lockHeight) return

	const height =
		el instanceof HTMLElement
			? el.offsetHeight
			: (contentFrameElement.value?.offsetHeight ?? host.value?.offsetHeight ?? 0)
	if (height > 0) {
		lockedHeight.value = `${height}px`
	}
}

function releaseHostHeight(delayMs = 0) {
	clearHeightReleaseTimer()

	if (delayMs > 0) {
		heightReleaseTimer = setTimeout(() => {
			heightReleaseTimer = undefined
			lockedHeight.value = undefined
		}, delayMs)
		return
	}

	lockedHeight.value = undefined
}

function getSafetyTimeoutMs() {
	return (
		Math.max(
			motion.value.safetyMs,
			transitionDuration.value.enter + transitionDuration.value.leave + props.incomingHoldMs,
		) +
		props.heightReleaseDelayMs +
		250
	)
}

function cancelActiveTransitionForSafety(
	token: number,
	phase: 'enter' | 'leave',
	el?: Element,
) {
	if (token !== transitionToken) return

	transitionToken++
	cancelContentFrameAnimation()
	clearFreezeFrame()
	releaseHostHeight()
	renderContent.value = props.visible
	setTransitioning(false)

	if (!el) return
	if (phase === 'enter') {
		emit('enter-cancelled', el)
	} else {
		emit('leave-cancelled', el)
	}
}

function scheduleSafetyCleanup(token: number, phase: 'enter' | 'leave', el?: Element) {
	clearSafetyTimer()
	safetyTimer = setTimeout(() => {
		safetyTimer = undefined
		cancelActiveTransitionForSafety(token, phase, el)
	}, getSafetyTimeoutMs())
}

function getFreezeFrameEnterTransform() {
	return motion.value.enabled && motion.value.type !== 'none'
		? getUiMotionEnterTransform(motion.value)
		: 'none'
}

function getFreezeFrameLeaveTransform() {
	return motion.value.enabled && motion.value.type !== 'none'
		? getUiMotionLeaveTransform(motion.value)
		: 'none'
}

function getContentFrameEnterStart() {
	return {
		opacity: fadesFrame.value ? 0 : 1,
		transform: getFreezeFrameEnterTransform(),
	}
}

function getContentFrameLeaveEnd() {
	return {
		opacity: fadesFrame.value ? 0 : 1,
		transform: getFreezeFrameLeaveTransform(),
	}
}

function resetContentFrameStyle(element: HTMLElement | null) {
	if (!element) return

	element.style.opacity = ''
	element.style.transform = ''
}

function setContentFrameStyle(
	element: HTMLElement,
	style: {
		opacity: number
		transform: string
	},
) {
	element.style.opacity = `${style.opacity}`
	element.style.transform = style.transform
}

function setContentFrameEnterStart(element: HTMLElement) {
	if (!motion.value.enabled || motion.value.type === 'none' || transitionDuration.value.enter <= 0) {
		resetContentFrameStyle(element)
		return
	}

	setContentFrameStyle(element, getContentFrameEnterStart())
}

async function playOutgoingFreezeFrameLeave() {
	const element = outgoingFreezeFrameElement.value
	if (!element) return

	outgoingFreezeFrameAnimation?.cancel()
	outgoingFreezeFrameAnimation = undefined

	const keyframes = [
		{ opacity: 1, transform: 'none' },
		{
			opacity: fadesFrame.value ? 0 : 1,
			transform: getFreezeFrameLeaveTransform(),
		},
	]
	const duration = transitionDuration.value.leave

	if (typeof element.animate === 'function' && duration > 0) {
		const animation = element.animate(keyframes, {
			duration,
			easing: motion.value.leaveEasing,
			fill: 'forwards',
		})
		outgoingFreezeFrameAnimation = animation

		try {
			await animation.finished
		} catch {
			return
		}

		return
	}

	const finalKeyframe = keyframes[keyframes.length - 1]
	element.style.opacity = `${finalKeyframe.opacity}`
	element.style.transform = `${finalKeyframe.transform}`
}

function captureFreezeFrame() {
	if (
		!motion.value.freezeLeave ||
		isUiMotionFreezeFrameHiddenForDebug() ||
		!motion.value.enabled ||
		motion.value.type === 'none'
	) {
		return false
	}

	const snapshot = props.createFreezeFrame?.()
	if (snapshot === null || snapshot === undefined) return false

	outgoingFreezeSnapshot.value = snapshot
	freezeFrameVisible.value = true
	return true
}

function prepareIncomingFreezeFrame(el?: Element) {
	if (!props.prepareIncomingFreezeFrame) return Promise.resolve()

	try {
		return Promise.resolve(props.prepareIncomingFreezeFrame(el))
	} catch {
		return Promise.resolve()
	}
}

function nextPaint() {
	if (typeof window === 'undefined' || typeof requestAnimationFrame === 'undefined') {
		return Promise.resolve()
	}

	return new Promise<void>((resolve) => {
		requestAnimationFrame(() => {
			setTimeout(resolve, 0)
		})
	})
}

function wait(ms: number) {
	return new Promise<void>((resolve) => {
		setTimeout(resolve, ms)
	})
}

async function waitForIncomingFirstPaint(token: number) {
	const holdMs = motion.value.enabled ? props.incomingHoldMs : 0
	const deadline = Date.now() + holdMs

	while (
		token === transitionToken &&
		props.isIncomingReady &&
		!props.isIncomingReady() &&
		Date.now() < deadline
	) {
		await wait(16)
	}

	await nextTick()
	await nextPaint()
}

async function runLeaveTransition() {
	const token = ++transitionToken
	const transitionElement = contentFrameElement.value ?? host.value ?? undefined

	cancelContentFrameAnimation()
	clearFreezeFrame()
	clearHeightReleaseTimer()
	lockHostHeight(transitionElement)

	const hasFreezeFrame = captureFreezeFrame()
	setTransitioning(true)
	scheduleSafetyCleanup(token, 'leave', transitionElement)
	if (transitionElement) emit('before-leave', transitionElement)

	try {
		await nextTick()
		await nextPaint()

		const outgoingMotion = hasFreezeFrame
			? playOutgoingFreezeFrameLeave()
			: transitionElement instanceof HTMLElement
				? playContentFrameLeave(transitionElement)
				: Promise.resolve()

		await outgoingMotion

		if (token !== transitionToken) return

		renderContent.value = false
		await nextTick()

		if (token !== transitionToken) return

		await prepareIncomingFreezeFrame(transitionElement)

		if (token !== transitionToken) return

		renderContent.value = true
		await nextTick()

		if (token !== transitionToken) return

		const incomingElement = contentFrameElement.value
		if (incomingElement instanceof HTMLElement) {
			setContentFrameEnterStart(incomingElement)
		}

		await waitForIncomingFirstPaint(token)

		if (token !== transitionToken) return
		if (transitionElement) emit('after-leave', transitionElement)
	} catch {
		if (token === transitionToken) {
			clearSafetyTimer()
			clearFreezeFrame()
			releaseHostHeight()
			setTransitioning(false)
			renderContent.value = props.visible
			if (transitionElement) emit('leave-cancelled', transitionElement)
		}
	}
}

async function playContentFrameLeave(element: HTMLElement) {
	if (!motion.value.enabled || motion.value.type === 'none' || transitionDuration.value.leave <= 0) {
		resetContentFrameStyle(element)
		return
	}

	contentFrameAnimation?.cancel()
	contentFrameAnimation = undefined

	const keyframes = [{ opacity: 1, transform: 'none' }, getContentFrameLeaveEnd()]
	const duration = transitionDuration.value.leave

	if (typeof element.animate === 'function') {
		const animation = element.animate(keyframes, {
			duration,
			easing: motion.value.leaveEasing,
			fill: 'forwards',
		})
		contentFrameAnimation = animation

		try {
			await animation.finished
		} catch {
			return
		}

		if (contentFrameAnimation === animation) {
			contentFrameAnimation = undefined
		}

		return
	}

	setContentFrameStyle(element, keyframes[keyframes.length - 1])
}

async function playContentFrameEnter(element: HTMLElement) {
	if (!motion.value.enabled || motion.value.type === 'none' || transitionDuration.value.enter <= 0) {
		clearFreezeFrame()
		resetContentFrameStyle(element)
		return
	}

	const keyframes = [getContentFrameEnterStart(), { opacity: 1, transform: 'none' }]

	setContentFrameStyle(element, keyframes[0])
	clearFreezeFrame()

	await nextPaint()

	if (typeof element.animate === 'function') {
		const animation = element.animate(keyframes, {
			duration: transitionDuration.value.enter,
			easing: motion.value.enterEasing,
			fill: 'forwards',
		})
		contentFrameAnimation = animation

		try {
			await animation.finished
		} catch {
			return
		}

		if (contentFrameAnimation === animation) {
			animation.cancel()
			contentFrameAnimation = undefined
		}
	}

	resetContentFrameStyle(element)
}

async function runEnterTransition() {
	const token = ++transitionToken

	clearHeightReleaseTimer()
	cancelContentFrameAnimation()
	renderContent.value = true
	setTransitioning(true)

	try {
		await nextTick()

		if (token !== transitionToken) return

		const transitionElement = contentFrameElement.value ?? host.value ?? undefined
		scheduleSafetyCleanup(token, 'enter', transitionElement)
		if (transitionElement) emit('before-enter', transitionElement)

		if (transitionElement instanceof HTMLElement) {
			await playContentFrameEnter(transitionElement)
		} else {
			clearFreezeFrame()
		}

		if (token !== transitionToken) return

		clearSafetyTimer()
		releaseHostHeight(props.heightReleaseDelayMs)
		setTransitioning(false)
		if (transitionElement) emit('after-enter', transitionElement)
	} catch {
		if (token === transitionToken) {
			clearSafetyTimer()
			clearFreezeFrame()
			releaseHostHeight()
			setTransitioning(false)
			const transitionElement = contentFrameElement.value ?? host.value ?? undefined
			if (transitionElement) emit('enter-cancelled', transitionElement)
		}
	} finally {
		if (token === transitionToken && !transitioningActive.value) {
			renderContent.value = props.visible
		}
	}
}

async function runContentKeyTransition() {
	const token = ++transitionToken
	const transitionElement = contentFrameElement.value ?? host.value ?? undefined

	cancelContentFrameAnimation()
	clearFreezeFrame()
	clearHeightReleaseTimer()
	lockHostHeight(transitionElement)

	const hasFreezeFrame = captureFreezeFrame()
	setTransitioning(true)
	scheduleSafetyCleanup(token, 'enter', transitionElement)
	if (transitionElement) emit('before-leave', transitionElement)

	const incomingPrep = prepareIncomingFreezeFrame(transitionElement)

	try {
		await nextTick()
		await nextPaint()

		const outgoingMotion = hasFreezeFrame
			? playOutgoingFreezeFrameLeave()
			: Promise.resolve()

		await Promise.all([incomingPrep, outgoingMotion])

		if (token !== transitionToken) return

		await waitForIncomingFirstPaint(token)

		if (token !== transitionToken) return
		if (transitionElement) emit('after-leave', transitionElement)

		const enterElement = contentFrameElement.value ?? host.value ?? undefined
		if (enterElement) emit('before-enter', enterElement)

		if (enterElement instanceof HTMLElement) {
			await playContentFrameEnter(enterElement)
		} else {
			clearFreezeFrame()
		}

		if (token !== transitionToken) return

		clearSafetyTimer()
		releaseHostHeight(props.heightReleaseDelayMs)
		setTransitioning(false)
		if (enterElement) emit('after-enter', enterElement)
	} catch {
		if (token === transitionToken) {
			clearSafetyTimer()
			clearFreezeFrame()
			releaseHostHeight()
			setTransitioning(false)
			renderContent.value = props.visible
			const enterElement = contentFrameElement.value ?? host.value ?? undefined
			if (enterElement) emit('enter-cancelled', enterElement)
		}
	} finally {
		if (token === transitionToken && !transitioningActive.value) {
			renderContent.value = props.visible
		}
	}
}

watch(
	() => props.visible,
	(visible) => {
		if (visible) {
			void runEnterTransition()
		} else {
			void runLeaveTransition()
		}
	},
)

watch(
	() => props.contentKey,
	(next, previous) => {
		if (next === previous || !props.visible || transitioningActive.value) return

		void runContentKeyTransition()
	},
	{ flush: 'pre' },
)

onUnmounted(() => {
	transitionToken++
	clearSafetyTimer()
	clearHeightReleaseTimer()
	cancelContentFrameAnimation()
	clearFreezeFrame()
	setTransitioning(false)
})
</script>

<style scoped>
.app-freeze-frame-transition {
	position: relative;
	isolation: isolate;
	width: 100%;
	min-height: 100%;
	min-width: 0;
}

.app-freeze-frame-transition__frame {
	position: relative;
	width: 100%;
	min-height: inherit;
	min-width: 0;
}

.app-freeze-frame-transition__frame--covered {
	visibility: hidden;
}

.app-freeze-frame-transition__freeze-cover {
	position: fixed;
	inset: 0;
	z-index: 10000;
	pointer-events: none;
	overflow: hidden;
}

.app-freeze-frame-transition__freeze-frame {
	position: fixed;
	inset: 0;
	pointer-events: none;
	will-change: opacity, transform;
}

.app-freeze-frame-transition__freeze-frame--outgoing {
	z-index: 2;
}
</style>
