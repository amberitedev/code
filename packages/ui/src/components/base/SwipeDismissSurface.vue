<template>
	<div
		ref="surface"
		class="swipe-dismiss-surface"
		:class="{
			'swipe-dismiss-surface--disabled': disabled,
			'swipe-dismiss-surface--dragging': dragging,
			'swipe-dismiss-surface--committed': committed,
			'swipe-dismiss-surface--snap-active': snapActive,
			'swipe-dismiss-surface--left': revealDirection > 0,
			'swipe-dismiss-surface--danger-muted': color === 'danger-muted',
		}"
		:aria-label="ariaLabel"
		:style="surfaceStyle"
		role="group"
		@pointerdown="onPointerDown"
	>
		<div class="swipe-dismiss-surface__reveal" aria-hidden="true">
			<div class="swipe-dismiss-surface__action swipe-dismiss-surface__action--left">
				<span class="swipe-dismiss-surface__x-icon">
					<span></span>
					<span></span>
				</span>
			</div>
		</div>
		<div
			class="swipe-dismiss-surface__content"
			:style="contentStyle"
		>
			<slot />
		</div>
	</div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

type AxisLock = 'undecided' | 'horizontal' | 'vertical'
type Sample = {
	x: number
	time: number
}

const props = withDefaults(
	defineProps<{
		disabled?: boolean
		threshold?: number
		axisLockThreshold?: number
		ariaLabel?: string
		color?: 'danger-muted'
	}>(),
	{
		disabled: false,
		threshold: 1 / 3,
		axisLockThreshold: 10,
		ariaLabel: 'Dismiss',
		color: 'danger-muted',
	},
)

const emit = defineEmits<{
	dismiss: []
	'drag-start': []
	'drag-end': []
	progress: [progress: number]
}>()

const SAMPLE_WINDOW_MS = 100
const TAP_DRAG_THRESHOLD = 4
const INTENT_RATIO = 1.12
const FLING_VELOCITY = 1200
const FLING_MIN_PROGRESS = 0.12
const SNAP_MS = 180
const COMMIT_MS = 500
const DETENT_SNAP_MS = 150
const DETENT_RELEASE_MS = 110
const USER_TRAVEL_PROGRESS = 1 / 3
const EXIT_AFTER_WALL_PROGRESS = 0.2
const DETENT_RESISTANCE = 0.08
const DETENT_RETURN_RESISTANCE = 0.25
const DETENT_RELEASE_PROGRESS = 0.08
const PRE_DETENT_PROGRESS = 0.08
const PRE_DETENT_RESISTANCE = 0.55
const BREAKAWAY_PROGRESS = 0.7

const surface = ref<HTMLElement | null>(null)
const offset = ref(0)
const axisLock = ref<AxisLock>('undecided')
const dragging = ref(false)
const committed = ref(false)
const snapActive = ref(false)
const prefersReducedMotion = ref(false)
const commitStartOffset = ref(0)

let startX = 0
let startY = 0
let activePointerId: number | null = null
let samples: Sample[] = []
let animationFrame: number | null = null
let activePointerTarget: HTMLElement | null = null
let mediaQuery: MediaQueryList | null = null
let gestureStartOffset = 0
let lastRawOffset = 0
let detentLatched = false
let detentAnchorRaw = 0

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value))
const easeOutCubic = (value: number) => 1 - Math.pow(1 - value, 3)
const easeSnapAssist = (value: number) => {
	const overshoot = 1.045
	const snapPoint = 0.72

	if (value < snapPoint) {
		return overshoot * Math.pow(value / snapPoint, 3)
	}

	const settle = (value - snapPoint) / (1 - snapPoint)
	return overshoot - (overshoot - 1) * easeOutCubic(settle)
}
const width = () => Math.max(1, surface.value?.clientWidth ?? 1)
const wallDistance = () => width() * USER_TRAVEL_PROGRESS
const exitDistance = () => width() * (USER_TRAVEL_PROGRESS + EXIT_AFTER_WALL_PROGRESS)
const snapDistance = () => width() * clamp(props.threshold, 0.05, USER_TRAVEL_PROGRESS)
const detentReleaseDistance = () => Math.max(0, snapDistance() - width() * DETENT_RELEASE_PROGRESS)
const preDetentDistance = () => Math.max(0, snapDistance() - width() * PRE_DETENT_PROGRESS)
const breakawayDistance = () =>
	wallDistance() + (exitDistance() - wallDistance()) * BREAKAWAY_PROGRESS

const revealDirection = computed(() => (offset.value > 0 ? 1 : 0))
const revealProgress = computed(() => clamp(offset.value / wallDistance(), 0, 1))
const revealWidth = computed(() => Math.min(offset.value, wallDistance()))
const fadeProgress = computed(() =>
	committed.value
		? clamp(
				(offset.value - commitStartOffset.value) /
					Math.max(1, exitDistance() - commitStartOffset.value),
				0,
				1,
			)
		: 0,
)
const surfaceStyle = computed(() => ({
	'--swipe-dismiss-offset': `${offset.value}px`,
	'--swipe-dismiss-reveal-width': `${revealWidth.value}px`,
	'--swipe-dismiss-wall': `${wallDistance()}px`,
	'--swipe-dismiss-exit': `${exitDistance()}px`,
	'--swipe-dismiss-progress': `${revealProgress.value}`,
	'--swipe-dismiss-content-opacity': `${1 - fadeProgress.value}`,
	'--swipe-dismiss-underlay-bg': 'var(--color-red)',
	'--swipe-dismiss-underlay-border': 'var(--color-red-highlight)',
}))
const contentStyle = computed(() => ({
	opacity: `var(--swipe-dismiss-content-opacity, 1)`,
	transform: `translate3d(var(--swipe-dismiss-offset, 0px), 0, 0)`,
}))

function resetSamples() {
	samples = []
}

function addSample(x: number, time: number) {
	samples.push({ x, time })
	samples = samples.filter((sample) => time - sample.time <= SAMPLE_WINDOW_MS)
}

function getVelocity(now: number) {
	const recent = samples.filter((sample) => now - sample.time <= SAMPLE_WINDOW_MS)
	const first = recent[0]
	const last = recent[recent.length - 1]
	if (!first || !last || last.time === first.time) return 0

	return (last.x - first.x) / ((last.time - first.time) / 1000)
}

function emitProgress() {
	emit('progress', clamp(offset.value / exitDistance(), 0, 1))
}

function getPreDetentOffset(raw: number) {
	const preDetent = preDetentDistance()
	if (raw <= preDetent) return raw

	return preDetent + (raw - preDetent) * PRE_DETENT_RESISTANCE
}

function interactiveOffset(value: number) {
	const raw = Math.max(value, 0)
	lastRawOffset = raw

	if (raw <= 0) {
		detentLatched = false
		detentAnchorRaw = 0
		snapActive.value = false
		return 0
	}

	const snap = snapDistance()
	const wall = wallDistance()

	if (detentLatched && raw <= detentReleaseDistance()) {
		detentLatched = false
		detentAnchorRaw = 0
		snapActive.value = false
		animateTo(getPreDetentOffset(raw), undefined, DETENT_RELEASE_MS)
		return offset.value
	}

	if (!detentLatched) {
		if (raw < snap) {
			return getPreDetentOffset(raw)
		}

		detentLatched = true
		detentAnchorRaw = raw
		snapActive.value = true
		animateTo(wall, undefined, DETENT_SNAP_MS, easeSnapAssist)
		return offset.value
	}

	const rawDelta = raw - detentAnchorRaw
	if (rawDelta >= 0) {
		const assistedOffset = wall + rawDelta * DETENT_RESISTANCE

		return animationFrame === null ? assistedOffset : Math.max(offset.value, assistedOffset)
	}

	const returnOffset = wall + rawDelta * DETENT_RETURN_RESISTANCE

	return animationFrame === null ? returnOffset : Math.min(offset.value, returnOffset)
}

function setOffset(value: number, allowExitTravel = false) {
	offset.value = clamp(value, 0, allowExitTravel ? exitDistance() : wallDistance())
	emitProgress()
}

function setInteractiveOffset(value: number) {
	offset.value = clamp(interactiveOffset(value), 0, exitDistance())
	emitProgress()
}

function resetDetent() {
	gestureStartOffset = 0
	lastRawOffset = 0
	detentLatched = false
	detentAnchorRaw = 0
	snapActive.value = false
}

function clearAnimation() {
	if (animationFrame === null) return

	cancelAnimationFrame(animationFrame)
	animationFrame = null
}

function releasePointer() {
	const target = activePointerTarget
	if (!target) return

	target.removeEventListener('pointermove', onPointerMove)
	target.removeEventListener('pointerup', onPointerEnd)
	target.removeEventListener('pointercancel', onPointerCancel)
	if (activePointerId !== null && target.hasPointerCapture(activePointerId)) {
		target.releasePointerCapture(activePointerId)
	}
	activePointerId = null
	activePointerTarget = null
}

function resetGesture() {
	axisLock.value = 'undecided'
	dragging.value = false
	resetSamples()
	releasePointer()
}

function animateTo(
	target: number,
	onDone?: () => void,
	durationMs = SNAP_MS,
	easing: (value: number) => number = easeOutCubic,
) {
	clearAnimation()

	const from = offset.value
	const duration = prefersReducedMotion.value ? 0 : durationMs
	if (duration === 0) {
		setOffset(target, true)
		onDone?.()
		return
	}

	const startedAt = performance.now()
	const tick = (now: number) => {
		const t = clamp((now - startedAt) / duration, 0, 1)
		setOffset(from + (target - from) * easing(t), true)
		if (t < 1) {
			animationFrame = requestAnimationFrame(tick)
			return
		}

		animationFrame = null
		onDone?.()
	}

	animationFrame = requestAnimationFrame(tick)
}

function resetToRest() {
	committed.value = false
	commitStartOffset.value = 0
	resetDetent()
	animateTo(0, () => {
		resetGesture()
	})
}

function commitDismiss() {
	commitStartOffset.value = offset.value
	committed.value = true
	animateTo(exitDistance(), () => {
		resetDetent()
		resetGesture()
		emit('dismiss')
	}, COMMIT_MS)
}

function finishDrag(velocity: number) {
	emit('drag-end')
	const projectedRaw = lastRawOffset + Math.max(velocity, 0) * 0.12
	const flungHardEnough =
		velocity >= FLING_VELOCITY && offset.value >= exitDistance() * FLING_MIN_PROGRESS
	if (
		lastRawOffset >= breakawayDistance() ||
		projectedRaw >= exitDistance() ||
		flungHardEnough
	) {
		commitDismiss()
		return
	}

	if (detentLatched || offset.value >= snapDistance()) {
		commitDismiss()
		return
	}

	resetToRest()
}

function startHandleDrag() {
	if (props.disabled) return false

	clearAnimation()
	committed.value = false
	commitStartOffset.value = 0
	axisLock.value = 'horizontal'
	dragging.value = true
	resetDetent()
	setOffset(0)
	emit('drag-start')
	return true
}

function moveHandleDrag(value: number) {
	if (!dragging.value && !startHandleDrag()) return

	setInteractiveOffset(value)
}

function releaseHandleDrag(velocity = 0) {
	if (!dragging.value) return

	finishDrag(velocity)
}

function cancelHandleDrag() {
	if (dragging.value) emit('drag-end')
	resetToRest()
}

function onPointerDown(event: PointerEvent) {
	if (props.disabled || (event.pointerType === 'mouse' && event.button !== 0)) return

	clearAnimation()
	committed.value = false
	commitStartOffset.value = 0
	startX = event.clientX
	startY = event.clientY
	gestureStartOffset = offset.value
	lastRawOffset = offset.value
	detentLatched = offset.value >= wallDistance() - 0.5
	detentAnchorRaw = detentLatched ? offset.value : 0
	activePointerId = event.pointerId
	activePointerTarget = surface.value
	axisLock.value = 'undecided'
	resetSamples()
	addSample(event.clientX, event.timeStamp)
	activePointerTarget?.setPointerCapture(event.pointerId)
	activePointerTarget?.addEventListener('pointermove', onPointerMove)
	activePointerTarget?.addEventListener('pointerup', onPointerEnd)
	activePointerTarget?.addEventListener('pointercancel', onPointerCancel)
}

function onPointerMove(event: PointerEvent) {
	if (activePointerId !== event.pointerId) return

	addSample(event.clientX, event.timeStamp)
	const deltaX = event.clientX - startX
	const deltaY = event.clientY - startY
	const absX = Math.abs(deltaX)
	const absY = Math.abs(deltaY)

	if (axisLock.value === 'undecided') {
		if (Math.hypot(deltaX, deltaY) < TAP_DRAG_THRESHOLD) return

		if (absX >= props.axisLockThreshold && absX > absY * INTENT_RATIO) {
			if (deltaX <= 0 && gestureStartOffset <= 0) {
				resetGesture()
				return
			}

			axisLock.value = 'horizontal'
			dragging.value = true
			emit('drag-start')
		} else if (absY >= props.axisLockThreshold && absY > absX) {
			axisLock.value = 'vertical'
			resetGesture()
			return
		} else {
			return
		}
	}

	if (axisLock.value !== 'horizontal') return

	event.preventDefault()
	setInteractiveOffset(gestureStartOffset + deltaX)
}

function onPointerEnd(event: PointerEvent) {
	if (activePointerId !== event.pointerId) return

	const wasDragging = dragging.value && axisLock.value === 'horizontal'
	const velocity = getVelocity(event.timeStamp)
	releasePointer()
	surface.value?.removeEventListener('pointermove', onPointerMove)
	surface.value?.removeEventListener('pointerup', onPointerEnd)
	surface.value?.removeEventListener('pointercancel', onPointerCancel)

	if (!wasDragging) {
		resetGesture()
		return
	}

	finishDrag(velocity)
}

function onPointerCancel(event: PointerEvent) {
	if (activePointerId !== event.pointerId) return

	surface.value?.removeEventListener('pointermove', onPointerMove)
	surface.value?.removeEventListener('pointerup', onPointerEnd)
	surface.value?.removeEventListener('pointercancel', onPointerCancel)
	if (dragging.value) emit('drag-end')
	resetToRest()
}

function syncReducedMotion(query: MediaQueryList | MediaQueryListEvent) {
	prefersReducedMotion.value = query.matches
}

watch(
	() => props.disabled,
	(disabled) => {
		if (disabled) {
			clearAnimation()
			offset.value = 0
			committed.value = false
			commitStartOffset.value = 0
			resetDetent()
			resetGesture()
			emitProgress()
		}
	},
)

onMounted(() => {
	if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return

	mediaQuery = window.matchMedia('(prefers-reduced-motion: reduce)')
	syncReducedMotion(mediaQuery)
	mediaQuery.addEventListener('change', syncReducedMotion)
})

onBeforeUnmount(() => {
	clearAnimation()
	surface.value?.removeEventListener('pointermove', onPointerMove)
	surface.value?.removeEventListener('pointerup', onPointerEnd)
	surface.value?.removeEventListener('pointercancel', onPointerCancel)
	mediaQuery?.removeEventListener('change', syncReducedMotion)
	resetGesture()
})

defineExpose({
	startHandleDrag,
	moveHandleDrag,
	releaseHandleDrag,
	cancelHandleDrag,
})
</script>

<style scoped lang="scss">
.swipe-dismiss-surface {
	position: relative;
	isolation: isolate;
	overflow: hidden;
	border-radius: inherit;
	touch-action: pan-y;
	user-select: none;
}

.swipe-dismiss-surface--disabled {
	pointer-events: none;
}

.swipe-dismiss-surface__reveal {
	position: absolute;
	top: 0;
	bottom: 0;
	left: 0;
	z-index: 0;
	width: var(--swipe-dismiss-reveal-width, 0px);
	max-width: var(--swipe-dismiss-wall, 100%);
	overflow: hidden;
	border-radius: inherit;
	background: var(--swipe-dismiss-underlay-bg, var(--surface-4));
	box-shadow:
		inset 0 0 0 1px var(--swipe-dismiss-underlay-border, var(--surface-5)),
		0 -0.5rem 1.75rem color-mix(in srgb, var(--color-bg) 52%, transparent);
	opacity: var(--swipe-dismiss-content-opacity, 1);
	pointer-events: none;
}

.swipe-dismiss-surface__action {
	position: absolute;
	top: 0;
	bottom: 0;
	display: flex;
	width: var(--swipe-dismiss-reveal-width, var(--swipe-dismiss-offset, 0px));
	min-width: 4.5rem;
	align-items: center;
	justify-content: center;
	z-index: 1;
	border: 0;
	background: transparent;
	color: var(--color-accent-contrast, white);
	opacity: clamp(0, calc(var(--swipe-dismiss-progress, 0) * 1.6), 1);
	overflow: hidden;
}

.swipe-dismiss-surface__x-icon {
	position: relative;
	display: block;
	flex: 0 0 auto;
	width: 3.25rem;
	height: 3.25rem;
	filter: drop-shadow(0 1px 2px color-mix(in srgb, black 32%, transparent));
	transform: scale(calc(0.86 + var(--swipe-dismiss-progress, 0) * 0.14));
	transform-origin: center;
}

.swipe-dismiss-surface__x-icon span {
	position: absolute;
	top: 50%;
	left: 50%;
	display: block;
	width: 3.1rem;
	height: 0.42rem;
	border-radius: 999px;
	background: white;
	transform-origin: center;
}

.swipe-dismiss-surface__x-icon span:first-child {
	transform: translate(-50%, -50%) rotate(45deg);
}

.swipe-dismiss-surface__x-icon span:last-child {
	transform: translate(-50%, -50%) rotate(-45deg);
}

.swipe-dismiss-surface--snap-active .swipe-dismiss-surface__x-icon {
	animation: swipe-dismiss-icon-pop 360ms cubic-bezier(0.16, 1.35, 0.28, 1) both;
}

.swipe-dismiss-surface__action--left {
	left: 0;
}

.swipe-dismiss-surface--left .swipe-dismiss-surface__action--left {
	opacity: 1;
}

.swipe-dismiss-surface__content {
	position: relative;
	z-index: 1;
	min-width: 0;
	opacity: var(--swipe-dismiss-content-opacity, 1);
	will-change: transform;
	touch-action: pan-y;
}

@keyframes swipe-dismiss-icon-pop {
	0% {
		transform: scale(0.62) rotate(-10deg);
	}

	58% {
		transform: scale(1.22) rotate(4deg);
	}

	100% {
		transform: scale(1) rotate(0);
	}
}

@media (prefers-reduced-motion: reduce) {
	.swipe-dismiss-surface--snap-active .swipe-dismiss-surface__x-icon {
		animation: none;
	}
}

</style>
