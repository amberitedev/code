<template>
	<div
		ref="shell"
		class="library-instance-pull-surface"
		:style="shellStyle"
		:class="{
			'is-active': active,
			'is-dragging': dragMode === 'dismiss',
			'is-minimized': minimized,
			'is-restoring': dragMode === 'restore',
			'is-opening': opening || dragMode === 'open' || dragMode === 'open-minimized',
			'is-closing': closing || dragMode === 'close',
		}"
	>
		<div
			v-if="showUnderlay"
			class="library-instance-pull-background"
			:class="{ visible: underlayVisible }"
			:style="backgroundStyle"
			aria-hidden="true"
			inert
		>
			<slot name="underlay" />
		</div>

		<div
			ref="frame"
			class="library-instance-pull-frame"
			:class="{ 'library-instance-pull-frame--motion-active': frameMotionActive }"
			:style="frameStyle"
			@pointerdown.capture="startHeaderDismissDrag"
		>
			<span v-if="showFrameNotch" class="library-instance-pull-notch" aria-hidden="true">
				<span class="library-instance-pull-handle"></span>
			</span>
			<slot name="content">
				<slot />
			</slot>
		</div>

		<button
			v-if="active"
			ref="dismissHandle"
			type="button"
			class="library-instance-pull-handle-target"
			:style="dismissHandleTargetStyle"
			aria-label="Pull down to return to library"
			@dblclick.prevent="handleDismissHandleDoubleClick"
			@pointerdown="startDismissDrag"
			@pointermove="updateDismissHandleProximity"
			@pointerleave="resetHandleProximity"
		></button>

		<SwipeDismissSurface
			v-if="minimized || dragMode === 'restore' || dragMode === 'open-minimized'"
			ref="dockedSwipe"
			class="library-instance-docked-swipe"
			:class="{ 'library-instance-docked-swipe--restoring': dockedSurfaceRestoring }"
			:style="restoreSurfaceStyle"
			:disabled="sideDismissDisabled"
			:threshold="0.24"
			aria-label="Dismiss minimized instance"
			@dismiss="commitSideDismiss"
			@drag-start="startSideDismiss"
			@drag-end="endSideDismiss"
			@progress="sideDismissProgress = $event"
		>
			<div class="library-instance-docked-frame" :style="dockedFrameStyle" aria-hidden="true" inert>
				<span class="library-instance-pull-notch" aria-hidden="true">
					<span class="library-instance-pull-handle"></span>
				</span>
				<div class="library-instance-docked-content">
					<div v-html="restoreSnapshotHtml"></div>
				</div>
			</div>
		</SwipeDismissSurface>

		<button
			v-if="minimized"
			ref="restoreHandle"
			type="button"
			class="library-instance-restore-drag-target"
			aria-label="Pull up to reopen instance"
			@click.prevent.stop="ignoreRestoreHandleClick"
			@dblclick.prevent.stop="handleRestoreHandleDoubleClick"
			@pointerdown.prevent.stop="startRestoreDrag"
		></button>

		<Teleport v-if="showNotchTuner" defer to="#sidebar-bottom-teleport-target">
			<div
				class="library-instance-notch-tuner"
				data-tauri-drag-region-exclude
				@click.stop
				@pointerdown.stop
				@pointerup.stop
			>
				<div class="library-instance-notch-tuner-header">
					<span>Notch length</span>
					<span>{{ notchWidthLabel }}</span>
				</div>
				<Slider
					id="library-instance-notch-width"
					v-model="notchWidthRem"
					:min="10"
					:max="28"
					:step="0.25"
					unit="rem"
				/>
				<div class="library-instance-notch-tuner-control">
					<label for="library-instance-snap-line">Snap travel</label>
					<DropdownSelect
						id="library-instance-snap-line"
						v-model="snapLineProgress"
						name="library-instance-snap-line"
						class="library-instance-snap-select"
						:options="snapLineOptions"
						:display-name="formatSnapLineOption"
						:max-visible-options="5"
						render-up
					>
						<span>{{ formatSnapLineOption(snapLineProgress) }}</span>
					</DropdownSelect>
				</div>
			</div>
		</Teleport>
	</div>
</template>

<script setup lang="ts">
import { DropdownSelect, Slider, SwipeDismissSurface } from '@modrinth/ui'
import { useStorage } from '@vueuse/core'
import { computed, onBeforeUnmount, ref, watch } from 'vue'

const props = withDefaults(
	defineProps<{
		active?: boolean
		minimized?: boolean
		opening?: boolean
		closing?: boolean
		dragDisabled?: boolean
		openKey?: string | number
		closeKey?: string | number
		restoreSnapshotHtml?: string
		sidebarVisible?: boolean
	}>(),
	{
		active: false,
		minimized: false,
		opening: false,
		closing: false,
		dragDisabled: false,
		openKey: 0,
		closeKey: 0,
		restoreSnapshotHtml: '',
		sidebarVisible: true,
	},
)

const emit = defineEmits<{
	(e: 'dismiss'): void
	(e: 'restore'): void
	(e: 'clear'): void
	(e: 'close-complete'): void
	(e: 'dragging', dragging: boolean): void
	(e: 'progress', progress: number, mode: 'dismiss' | 'restore' | 'open' | 'close' | 'none'): void
}>()

const FRICTION = 0.02
const SAMPLE_WINDOW_MS = 100
const TAP_DRAG_THRESHOLD = 5
const TOUCH_VELOCITY_BOOST = 1.5
const DISMISS_TRANSITION_MS = 260
const RESTORE_TRANSITION_MS = 260
const NOTCH_EXPOSURE = 112
const DISMISS_FLING_VELOCITY = 1850
const DISMISS_FLING_MIN_OFFSET = 140
const RESTORE_FLING_VELOCITY = -1700
const RESTORE_AXIS_THRESHOLD = 10
const RESTORE_AXIS_RATIO = 1.18
const DEFAULT_SNAP_LINE_PROGRESS = 0.5
const snapLineOptions = Object.freeze([0.33, 0.4, 0.5, 0.6, 0.67])

type DragMode = 'none' | 'dismiss' | 'restore' | 'open' | 'open-minimized' | 'close'
type RestoreAxis = 'undecided' | 'vertical' | 'horizontal'
type Sample = {
	x: number
	y: number
	time: number
}

const shell = ref<HTMLElement | null>(null)
const frame = ref<HTMLElement | null>(null)
const dismissHandle = ref<HTMLElement | null>(null)
const restoreHandle = ref<HTMLElement | null>(null)
const dockedSwipe = ref<{
	startHandleDrag: () => boolean
	moveHandleDrag: (value: number) => void
	releaseHandleDrag: (velocity?: number) => void
	cancelHandleDrag: () => void
} | null>(null)
const dragMode = ref<DragMode>('none')
const restoreAxis = ref<RestoreAxis>('undecided')
const dragOffset = ref(0)
const restoreOffset = ref(0)
const handleProximity = ref(0)
const animatingFrame = ref(false)
const animatingRestore = ref(false)
const sideDismissProgress = ref(0)
const openingHoldActive = ref(false)
const notchWidthRem = useStorage('app-library-instance-pull-notch-width-rem', 18)
const snapLineProgress = useStorage(
	'app-library-instance-pull-snap-line-progress',
	DEFAULT_SNAP_LINE_PROGRESS,
)

let activePointerTarget: HTMLElement | null = null
let startX = 0
let startY = 0
let startOffset = 0
let moved = false
let pendingDragMode: DragMode = 'none'
let restoreSideDragStarted = false
let samples: Sample[] = []
let animationFrame: number | null = null

const viewportHeight = () =>
	(shell.value?.closest('.app-viewport') as HTMLElement | null)?.clientHeight ||
	shell.value?.clientHeight ||
	window.innerHeight ||
	1
const dismissedOffset = () => Math.max(0, viewportHeight() - NOTCH_EXPOSURE)
const dismissCommitOffset = () => dismissedOffset() * normalizedSnapLineProgress.value
const restoreCommitOffset = () => dismissedOffset() * (1 - normalizedSnapLineProgress.value)
const projectDistance = (velocity: number) => -velocity / Math.log(FRICTION)
const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value))
const easeOutCubic = (value: number) => 1 - Math.pow(1 - value, 3)
const minimizedProgressForOffset = (offset: number) =>
	clamp(offset / Math.max(1, dismissedOffset()), 0, 1)

const prefersReducedMotion = () =>
	typeof window !== 'undefined' &&
	typeof window.matchMedia === 'function' &&
	window.matchMedia('(prefers-reduced-motion: reduce)').matches

const dockedSurfaceRestoring = computed(
	() =>
		dragMode.value === 'restore' ||
		dragMode.value === 'open-minimized' ||
		(props.minimized && (props.opening || restoreOffset.value <= 1)),
)
const surfaceProgress = computed(() => {
	if (dockedSurfaceRestoring.value && dragMode.value === 'none') return 0
	if (dragMode.value === 'restore' || dragMode.value === 'open-minimized') {
		return minimizedProgressForOffset(restoreOffset.value)
	}
	if (props.minimized) return 1
	return minimizedProgressForOffset(dragOffset.value)
})
const normalizedSnapLineProgress = computed(() =>
	clamp(Number(snapLineProgress.value) || DEFAULT_SNAP_LINE_PROGRESS, 0.2, 0.8),
)
const frameRevealProgress = computed(() => {
	if (props.opening && dragMode.value === 'none') return 1
	if (dragMode.value === 'open') return surfaceProgress.value
	return surfaceProgress.value
})
const actionOpacity = computed(() => 1 - clamp((surfaceProgress.value - 0.55) / 0.25, 0, 1))
const headerDividerColor = computed(
	() =>
		`color-mix(in srgb, var(--color-divider, var(--surface-5)) ${actionOpacity.value * 100}%, transparent)`,
)
const frameOutlineOpacity = computed(() => clamp((surfaceProgress.value - 0.45) / 0.35, 0, 1))
const frameNotchOpacity = computed(() =>
	!props.dragDisabled &&
	(props.active || props.opening || dragMode.value === 'dismiss' || dragMode.value === 'open')
		? 1
		: 0,
)
const dockedChromeProgress = computed(() => clamp(surfaceProgress.value, 0, 1))
const dockedNotchOpacity = computed(() =>
	props.dragDisabled ? 0 : dockedSurfaceRestoring.value ? 1 : dockedChromeProgress.value,
)
const dockedChromeRadius = computed(() =>
	dockedChromeProgress.value < 0.02 ? '0' : 'var(--radius-xl) var(--radius-xl) 0 0',
)
const dockedOutlineColor = computed(
	() =>
		`color-mix(in srgb, var(--surface-5) ${dockedChromeProgress.value * 100}%, transparent)`,
)
const dockedShadowColor = computed(
	() =>
		`color-mix(in srgb, var(--color-bg) ${dockedChromeProgress.value * 58}%, transparent)`,
)
const notchHover = computed(() => clamp(handleProximity.value, 0, 1))
const notchWidthLabel = computed(() => `${Number(notchWidthRem.value).toFixed(2)}rem`)
const showNotchTuner = computed(() => props.active || props.minimized || dragMode.value !== 'none')
const frameMotionActive = computed(
	() =>
		openingHoldActive.value ||
		dragMode.value === 'dismiss' ||
		dragMode.value === 'open' ||
		dragMode.value === 'close' ||
		animatingFrame.value ||
		Math.abs(dragOffset.value) > 0.1,
)
const showUnderlay = computed(
	() =>
		props.active ||
		props.minimized ||
		props.opening ||
		props.closing ||
		dragMode.value === 'dismiss' ||
		dragMode.value === 'restore' ||
		dragMode.value === 'open' ||
		dragMode.value === 'open-minimized' ||
		dragMode.value === 'close',
)
const underlayVisible = computed(
	() =>
		showUnderlay.value &&
		(props.active ||
			props.minimized ||
			props.opening ||
			props.closing ||
			frameRevealProgress.value > 0.01),
)
const showFrameNotch = computed(
	() =>
		!props.dragDisabled &&
		(props.active || props.opening || dragMode.value === 'dismiss' || dragMode.value === 'open'),
)
const sideDismissDisabled = computed(
	() =>
		props.dragDisabled ||
		props.active ||
		!props.minimized ||
		dragMode.value === 'dismiss' ||
		dragMode.value === 'open' ||
		dragMode.value === 'open-minimized' ||
		dragMode.value === 'close' ||
		(dragMode.value === 'restore' && restoreAxis.value === 'vertical'),
)

const frameOffset = computed(() => {
	if (openingHoldActive.value && dragMode.value === 'none') return dismissedOffset()

	return dragOffset.value
})

const frameStyle = computed(() => ({
	transform: frameMotionActive.value ? `translate3d(0, ${frameOffset.value}px, 0)` : undefined,
	borderRadius: frameOffset.value > 2 ? 'var(--radius-xl) var(--radius-xl) 0 0' : undefined,
	boxShadow:
		frameOffset.value > 2
			? `0 0 0 ${frameOutlineOpacity.value}px var(--surface-5), 0 -12px 36px color-mix(in srgb, var(--color-bg) 56%, transparent)`
			: undefined,
	'--library-instance-actions-opacity': `${actionOpacity.value}`,
	'--library-instance-actions-pointer-events': actionOpacity.value < 0.05 ? 'none' : 'auto',
	'--library-instance-header-divider-color': headerDividerColor.value,
	'--library-instance-motion-progress': `${surfaceProgress.value}`,
	'--library-instance-notch-progress': `${surfaceProgress.value}`,
	'--library-instance-notch-opacity': `${frameNotchOpacity.value}`,
	'--library-instance-notch-hover': `${notchHover.value}`,
}))

const backgroundStyle = computed(() => ({
	opacity: `${frameRevealProgress.value}`,
}))

const dismissHandleTargetStyle = computed(() => ({
	transform: `translate3d(-50%, ${dragOffset.value}px, 0)`,
}))

const restoreSurfaceStyle = computed(() => ({
	opacity: '1',
	transform: `translate3d(0, ${restoreOffset.value}px, 0)`,
	'--library-instance-actions-opacity': `${actionOpacity.value}`,
	'--library-instance-actions-pointer-events': actionOpacity.value < 0.05 ? 'none' : 'auto',
	'--library-instance-header-divider-color': headerDividerColor.value,
	'--library-instance-motion-progress': `${surfaceProgress.value}`,
	'--library-instance-docked-chrome-progress': `${dockedChromeProgress.value}`,
	'--library-instance-docked-radius': dockedChromeRadius.value,
	'--library-instance-docked-outline-color': dockedOutlineColor.value,
	'--library-instance-docked-shadow-color': dockedShadowColor.value,
	'--library-instance-notch-progress': `${surfaceProgress.value}`,
	'--library-instance-notch-opacity': `${dockedNotchOpacity.value}`,
	'--library-instance-notch-hover': `${dragMode.value === 'restore' ? 1 : 0}`,
}))

const dockedFrameStyle = computed(() => ({
	'--library-instance-actions-opacity': `${actionOpacity.value}`,
	'--library-instance-actions-pointer-events': actionOpacity.value < 0.05 ? 'none' : 'auto',
	'--library-instance-header-divider-color': headerDividerColor.value,
	'--library-instance-motion-progress': `${surfaceProgress.value}`,
	'--library-instance-docked-chrome-progress': `${dockedChromeProgress.value}`,
	'--library-instance-docked-radius': dockedChromeRadius.value,
	'--library-instance-docked-outline-color': dockedOutlineColor.value,
	'--library-instance-docked-shadow-color': dockedShadowColor.value,
}))

const shellStyle = computed(() => ({
	'--library-instance-pull-right-offset': props.sidebarVisible
		? 'var(--right-bar-width, 300px)'
		: '0px',
	'--library-instance-pull-content-width':
		'calc(100vw - var(--left-bar-width, 4rem) - var(--library-instance-pull-right-offset, 0px))',
	'--library-instance-pull-center-x':
		'calc(var(--left-bar-width, 4rem) + var(--library-instance-pull-content-width) / 2)',
	'--library-instance-notch-width': `${notchWidthRem.value}rem`,
	'--library-instance-handle-width': `calc(${notchWidthRem.value}rem * 0.34)`,
	'--library-instance-notch-hit-width': `calc(${notchWidthRem.value}rem + 3rem)`,
	'--library-instance-notch-exposure': `${NOTCH_EXPOSURE}px`,
}))

function resetSamples() {
	samples = []
}

function formatSnapLineOption(option: number | string) {
	const value = clamp(Number(option) || DEFAULT_SNAP_LINE_PROGRESS, 0.2, 0.8)
	return `${Math.round(value * 100)}% travel`
}

function addSample(x: number, y: number, time: number) {
	samples.push({ x, y, time })
	samples = samples.filter((sample) => time - sample.time <= SAMPLE_WINDOW_MS)
}

function getVelocity(now: number, axis: 'x' | 'y') {
	const recent = samples.filter((sample) => now - sample.time <= SAMPLE_WINDOW_MS)
	const first = recent[0]
	const last = recent[recent.length - 1]
	if (!first || !last || last.time === first.time) return 0

	return (last[axis] - first[axis]) / ((last.time - first.time) / 1000)
}

function dampDismissOffset(rawOffset: number) {
	if (rawOffset <= 0) return rawOffset * 0.16

	const max = dismissedOffset()
	if (rawOffset <= max) return rawOffset

	return max
}

function releaseActivePointer() {
	if (!activePointerTarget) return

	activePointerTarget.removeEventListener('pointermove', onPointerMove)
	activePointerTarget.removeEventListener('pointerup', onPointerEnd)
	activePointerTarget.removeEventListener('pointercancel', onPointerCancel)
	activePointerTarget = null
}

function clearAnimation() {
	if (animationFrame === null) return

	cancelAnimationFrame(animationFrame)
	animationFrame = null
}

function animateOffset(
	from: number,
	to: number,
	duration: number,
	update: (value: number) => void,
	onDone?: () => void,
) {
	clearAnimation()

	if (prefersReducedMotion() || duration <= 0) {
		update(to)
		onDone?.()
		return
	}

	const startedAt = performance.now()
	const tick = (now: number) => {
		const t = clamp((now - startedAt) / duration, 0, 1)
		update(from + (to - from) * easeOutCubic(t))
		if (t < 1) {
			animationFrame = requestAnimationFrame(tick)
			return
		}

		animationFrame = null
		onDone?.()
	}

	animationFrame = requestAnimationFrame(tick)
}

function startDismissDrag(event: PointerEvent) {
	if (!props.active || (event.pointerType === 'mouse' && event.button !== 0)) return

	event.preventDefault()
	startDrag(event, 'dismiss', dismissHandle.value)
}

function handleDismissHandleDoubleClick() {
	if (!props.active || dragMode.value !== 'none') return

	clearAnimation()
	dragMode.value = 'dismiss'
	dragOffset.value = 0
	animatingFrame.value = false
	emit('dragging', true)
	emitProgress()
	commitDismiss()
}

function ignoreRestoreHandleClick(event: MouseEvent) {
	event.preventDefault()
	event.stopPropagation()
}

function handleRestoreHandleDoubleClick(event?: MouseEvent) {
	event?.preventDefault()
	event?.stopPropagation()

	if (!props.minimized || dragMode.value !== 'none') return

	clearAnimation()
	dragMode.value = 'restore'
	restoreAxis.value = 'vertical'
	restoreOffset.value = dismissedOffset()
	animatingRestore.value = false
	handleProximity.value = 1
	emit('dragging', true)
	emitProgress()
	commitRestore()
}

function startHeaderDismissDrag(event: PointerEvent) {
	if (
		!props.active ||
		dragMode.value !== 'none' ||
		(event.pointerType === 'mouse' && event.button !== 0)
	) {
		return
	}

	const target = event.target instanceof Element ? event.target : null
	if (!target || !isHeaderDismissDragTarget(target)) return

	event.preventDefault()
	startDrag(event, 'dismiss', frame.value)
}

function isHeaderDismissDragTarget(target: Element) {
	if (target.closest('button, a, input, select, textarea, [role="button"], [data-tauri-drag-region]')) {
		return false
	}

	const routeRoot = target.closest('[data-library-instance-title]')
	if (!routeRoot) return false

	let current: Element | null = target
	while (current && current !== routeRoot) {
		if (
			current.classList.contains('flex-wrap') &&
			current.classList.contains('items-start') &&
			current.classList.contains('gap-4')
		) {
			return true
		}

		current = current.parentElement
	}

	return false
}

function startRestoreDrag(event: PointerEvent) {
	if (
		!props.minimized ||
		dragMode.value !== 'none' ||
		pendingDragMode !== 'none' ||
		(event.pointerType === 'mouse' && event.button !== 0)
	) {
		return
	}

	event.preventDefault()
	event.stopPropagation()
	startPendingRestoreDrag(event)
}

function startPendingRestoreDrag(event: PointerEvent) {
	clearAnimation()
	releaseActivePointer()
	resetSamples()

	pendingDragMode = 'restore'
	restoreAxis.value = 'undecided'
	restoreSideDragStarted = false
	startX = event.clientX
	startY = event.clientY
	startOffset = dismissedOffset()
	moved = false
	animatingRestore.value = false
	addSample(event.clientX, event.clientY, event.timeStamp)

	activePointerTarget = restoreHandle.value
	activePointerTarget?.setPointerCapture(event.pointerId)
	activePointerTarget?.addEventListener('pointermove', onPointerMove)
	activePointerTarget?.addEventListener('pointerup', onPointerEnd)
	activePointerTarget?.addEventListener('pointercancel', onPointerCancel)
}

function startDrag(event: PointerEvent, mode: DragMode, target: HTMLElement | null) {
	clearAnimation()
	releaseActivePointer()
	resetSamples()

	pendingDragMode = 'none'
	dragMode.value = mode
	restoreAxis.value = 'undecided'
	restoreSideDragStarted = false
	startX = event.clientX
	startY = event.clientY
	startOffset = mode === 'restore' ? restoreOffset.value : dragOffset.value
	moved = false
	animatingFrame.value = false
	animatingRestore.value = false
	handleProximity.value = 1
	addSample(event.clientX, event.clientY, event.timeStamp)
	emit('dragging', true)
	emitProgress()

	activePointerTarget = target
	activePointerTarget?.setPointerCapture(event.pointerId)
	activePointerTarget?.addEventListener('pointermove', onPointerMove)
	activePointerTarget?.addEventListener('pointerup', onPointerEnd)
	activePointerTarget?.addEventListener('pointercancel', onPointerCancel)
}

function onPointerMove(event: PointerEvent) {
	addSample(event.clientX, event.clientY, event.timeStamp)
	const deltaX = event.clientX - startX
	const deltaY = event.clientY - startY
	if (!moved && Math.hypot(deltaX, deltaY) < TAP_DRAG_THRESHOLD) return

	if (!moved && pendingDragMode !== 'none') {
		activatePendingDrag()
	}

	moved = true
	if (dragMode.value === 'dismiss') {
		dragOffset.value = dampDismissOffset(startOffset + deltaY)
	} else if (dragMode.value === 'restore') {
		updateRestoreDrag(deltaX, deltaY)
	}
	emitProgress()
}

function activatePendingDrag() {
	dragMode.value = pendingDragMode
	pendingDragMode = 'none'
	restoreOffset.value = startOffset
	animatingFrame.value = false
	animatingRestore.value = false
	handleProximity.value = 1
	emit('dragging', true)
}

function updateRestoreDrag(deltaX: number, deltaY: number) {
	if (restoreAxis.value === 'undecided') {
		const verticalIntent =
			Math.abs(deltaY) >= RESTORE_AXIS_THRESHOLD &&
			Math.abs(deltaY) > Math.abs(deltaX) * RESTORE_AXIS_RATIO
		const horizontalIntent =
			Math.abs(deltaX) >= RESTORE_AXIS_THRESHOLD &&
			Math.abs(deltaX) > Math.abs(deltaY) * RESTORE_AXIS_RATIO

		if (verticalIntent) {
			restoreAxis.value = 'vertical'
		} else if (horizontalIntent) {
			restoreAxis.value = 'horizontal'
			restoreOffset.value = dismissedOffset()
		} else {
			return
		}
	}

	if (restoreAxis.value === 'horizontal') {
		updateRestoreSideDrag(deltaX)
		return
	}

	restoreOffset.value = clamp(startOffset + deltaY, 0, dismissedOffset())
}

function updateRestoreSideDrag(deltaX: number) {
	restoreOffset.value = dismissedOffset()
	if (deltaX <= 0) {
		if (restoreSideDragStarted) {
			dockedSwipe.value?.moveHandleDrag(0)
		}
		return
	}

	if (!restoreSideDragStarted) {
		restoreSideDragStarted = dockedSwipe.value?.startHandleDrag() === true
	}

	if (restoreSideDragStarted) {
		dockedSwipe.value?.moveHandleDrag(deltaX)
	}
}

function onPointerEnd(event: PointerEvent) {
	const rawVelocityY = getVelocity(event.timeStamp, 'y')
	const velocityY = rawVelocityY * (event.pointerType === 'mouse' ? 1 : TOUCH_VELOCITY_BOOST)
	const mode = dragMode.value

	releaseActivePointer()
	resetHandleProximity()

	if (!moved) {
		if (pendingDragMode !== 'none') {
			resetPendingDragState()
		} else {
			resetDragState()
		}
		return
	}

	if (mode === 'dismiss') {
		finishDismissDrag(velocityY)
	} else if (mode === 'restore') {
		const rawVelocityX = getVelocity(event.timeStamp, 'x')
		const velocityX = rawVelocityX * (event.pointerType === 'mouse' ? 1 : TOUCH_VELOCITY_BOOST)
		finishRestoreDrag(velocityY, velocityX)
	}
}

function onPointerCancel() {
	releaseActivePointer()
	resetHandleProximity()
	if (pendingDragMode !== 'none') {
		resetPendingDragState()
	} else if (dragMode.value === 'restore' && restoreAxis.value === 'horizontal') {
		cancelRestoreSideDrag()
	} else if (dragMode.value === 'restore') {
		animateRestoreTo(dismissedOffset(), resetDragState)
	} else {
		animateFrameTo(0, resetDragState)
	}
}

function finishDismissDrag(velocity: number) {
	const projectedOffset = dragOffset.value + projectDistance(velocity)
	const pulledFarEnough = dragOffset.value >= dismissCommitOffset()
	const flungHardEnough =
		velocity > DISMISS_FLING_VELOCITY &&
		dragOffset.value > DISMISS_FLING_MIN_OFFSET &&
		projectedOffset >= dismissCommitOffset()

	if (pulledFarEnough || flungHardEnough) {
		commitDismiss()
		return
	}

	animateFrameTo(0, resetDragState)
}

function finishRestoreDrag(velocity: number, velocityX = 0) {
	if (restoreAxis.value === 'horizontal') {
		finishRestoreSideDrag(velocityX)
		return
	}

	const projectedOffset = restoreOffset.value + projectDistance(velocity)
	const pulledFarEnough = restoreOffset.value <= restoreCommitOffset()
	const flungHardEnough =
		velocity < RESTORE_FLING_VELOCITY && projectedOffset <= restoreCommitOffset()

	if (pulledFarEnough || flungHardEnough) {
		commitRestore()
		return
	}

	animateRestoreTo(dismissedOffset(), resetDragState)
}

function finishRestoreSideDrag(velocity: number) {
	if (restoreSideDragStarted) {
		dockedSwipe.value?.releaseHandleDrag(velocity)
	} else {
		dockedSwipe.value?.cancelHandleDrag()
	}

	dragMode.value = 'none'
	restoreAxis.value = 'undecided'
	restoreSideDragStarted = false
	restoreOffset.value = dismissedOffset()
	resetSamples()
	resetHandleProximity()
	emit('dragging', false)
	emit('progress', 0, 'none')
}

function cancelRestoreSideDrag() {
	dockedSwipe.value?.cancelHandleDrag()
	dragMode.value = 'none'
	restoreAxis.value = 'undecided'
	restoreSideDragStarted = false
	restoreOffset.value = dismissedOffset()
	resetSamples()
	resetHandleProximity()
	emit('dragging', false)
	emit('progress', 0, 'none')
}

function animateFrameTo(offset: number, onDone?: () => void) {
	animatingFrame.value = true
	animateOffset(
		dragOffset.value,
		offset,
		DISMISS_TRANSITION_MS,
		(value) => {
			dragOffset.value = value
			emitProgress()
		},
		() => {
			animatingFrame.value = false
			onDone?.()
		},
	)
}

function animateRestoreTo(offset: number, onDone?: () => void) {
	animatingRestore.value = true
	animateOffset(
		restoreOffset.value,
		offset,
		RESTORE_TRANSITION_MS,
		(value) => {
			restoreOffset.value = value
			emitProgress()
		},
		() => {
			animatingRestore.value = false
			onDone?.()
		},
	)
}

function commitDismiss() {
	animateFrameTo(dismissedOffset(), () => {
		emit('progress', 1, 'dismiss')
		emit('dismiss')
		animatingFrame.value = false
		resetSamples()
		resetHandleProximity()
		emit('dragging', false)
	})
}

function commitSideDismiss() {
	emit('clear')
	sideDismissProgress.value = 0
	resetDragState(false)
}

function commitRestore() {
	animateRestoreTo(0, () => {
		emit('progress', 0, 'restore')
		emit('restore')
		dragMode.value = 'none'
		animatingRestore.value = false
		resetSamples()
		resetHandleProximity()
		emit('dragging', false)
	})
}

function startOpenAnimation() {
	if (dragMode.value !== 'none' && dragMode.value !== 'restore') return

	if (props.minimized) {
		dragMode.value = 'open-minimized'
		restoreAxis.value = 'vertical'
		restoreOffset.value = dismissedOffset()
		animatingRestore.value = false
		emit('progress', 1, 'open')
		animateRestoreTo(0, () => {
			dragMode.value = 'none'
			restoreAxis.value = 'undecided'
			restoreSideDragStarted = false
			animatingRestore.value = false
			resetSamples()
			resetHandleProximity()
			emit('dragging', false)
			emit('progress', 0, 'none')
		})
		return
	}

	dragMode.value = 'open'
	openingHoldActive.value = false
	dragOffset.value = dismissedOffset()
	emit('progress', 1, 'open')
	animateFrameTo(0, resetDragState)
}

function startCloseAnimation() {
	if (dragMode.value !== 'none') return

	dragMode.value = 'close'
	dragOffset.value = 0
	emit('progress', 0, 'close')
	animateFrameTo(dismissedOffset(), () => {
		emit('progress', 1, 'close')
		emit('close-complete')
		animatingFrame.value = false
		resetSamples()
		resetHandleProximity()
		emit('dragging', false)
	})
}

function resetDragState(emitNone = true) {
	pendingDragMode = 'none'
	dragMode.value = 'none'
	restoreAxis.value = 'undecided'
	restoreSideDragStarted = false
	dragOffset.value = 0
	restoreOffset.value = dismissedOffset()
	openingHoldActive.value = false
	animatingFrame.value = false
	animatingRestore.value = false
	sideDismissProgress.value = 0
	resetSamples()
	resetHandleProximity()
	emit('dragging', false)
	if (emitNone) {
		emit('progress', 0, 'none')
	}
}

function resetPendingDragState() {
	pendingDragMode = 'none'
	restoreAxis.value = 'undecided'
	restoreSideDragStarted = false
	moved = false
	resetSamples()
	resetHandleProximity()
}

function startSideDismiss() {
	emit('dragging', true)
}

function endSideDismiss() {
	emit('dragging', false)
}

function updateDismissHandleProximity(event: PointerEvent) {
	if (dragMode.value !== 'none') return

	const handle = dismissHandle.value
	if (!handle) return

	const rect = handle.getBoundingClientRect()
	const dx = Math.max(rect.left - event.clientX, 0, event.clientX - rect.right)
	const dy = Math.max(rect.top - event.clientY, 0, event.clientY - rect.bottom)
	const distance = Math.hypot(dx, dy)
	handleProximity.value = distance <= 6 ? 1 : 1 - clamp((distance - 6) / 28, 0, 1)
}

function resetHandleProximity() {
	if (dragMode.value === 'none') {
		handleProximity.value = 0
	}
}

function emitProgress() {
	if (dragMode.value === 'dismiss' || dragMode.value === 'open' || dragMode.value === 'close') {
		emit('progress', minimizedProgressForOffset(dragOffset.value), dragMode.value)
	} else if (dragMode.value === 'open-minimized') {
		emit('progress', minimizedProgressForOffset(restoreOffset.value), 'open')
	} else if (dragMode.value === 'restore') {
		emit('progress', minimizedProgressForOffset(restoreOffset.value), 'restore')
	}
}

watch(
	() => props.minimized,
	() => {
		restoreOffset.value = dismissedOffset()
		restoreAxis.value = 'undecided'
		sideDismissProgress.value = 0
	},
	{ immediate: true },
)

watch(
	() => props.active,
	(active, previousActive) => {
		if (!active && ['dismiss', 'open', 'close'].includes(dragMode.value)) {
			resetDragState(false)
		} else if (!active && previousActive === true && dragMode.value === 'none' && !props.closing) {
			resetDragState(false)
		}
	},
)

watch(
	() => props.openKey,
	() => {
		if (props.opening || props.active) startOpenAnimation()
	},
)

watch(
	() => props.opening,
	(opening) => {
		if (opening && !props.active && dragMode.value === 'none') {
			openingHoldActive.value = true
		} else if (!opening) {
			openingHoldActive.value = false
		}
	},
)

watch(
	() => props.closeKey,
	() => {
		if (props.closing) startCloseAnimation()
	},
)

watch(
	() => props.closing,
	(closing) => {
		if (!closing && dragMode.value === 'close') resetDragState(false)
	},
)

onBeforeUnmount(() => {
	releaseActivePointer()
	clearAnimation()
})
</script>

<style scoped lang="scss">
.library-instance-pull-surface {
	position: relative;
	min-height: 100%;
	height: 100%;
	isolation: isolate;
}

.library-instance-pull-surface.is-dragging,
.library-instance-pull-surface.is-restoring,
.library-instance-pull-surface.is-opening,
.library-instance-pull-surface.is-closing {
	overflow: hidden;
}

.library-instance-pull-frame {
	position: relative;
	z-index: 2;
	min-height: 100%;
	height: 100%;
	background: var(--color-bg);
}

.library-instance-pull-frame--motion-active {
	overflow: hidden;
	will-change: transform;
}

.library-instance-pull-frame
	:deep(
		[data-library-instance-title] .flex.min-w-0.flex-1.gap-4 + .flex.flex-wrap.gap-2.items-center
	) {
	opacity: var(--library-instance-actions-opacity, 1);
	pointer-events: var(--library-instance-actions-pointer-events, auto);
}

.library-instance-docked-frame
	:deep(
		[data-library-instance-title] .flex.min-w-0.flex-1.gap-4 + .flex.flex-wrap.gap-2.items-center
	) {
	opacity: var(--library-instance-actions-opacity, 1);
	pointer-events: var(--library-instance-actions-pointer-events, auto);
}

.library-instance-pull-frame
	:deep(
		[data-library-instance-title]
			.flex.flex-col.gap-2.border-0.border-b.border-solid.border-divider.pb-4
	),
.library-instance-docked-frame
	:deep(
		[data-library-instance-title]
			.flex.flex-col.gap-2.border-0.border-b.border-solid.border-divider.pb-4
	) {
	border-bottom-color: var(--library-instance-header-divider-color, var(--color-divider));
}

.library-instance-pull-background {
	position: absolute;
	inset: 0;
	z-index: 1;
	min-height: 100%;
	background: var(--color-bg);
	opacity: 0;
	overflow: hidden;
	pointer-events: none;
	transform-origin: top center;
	will-change: opacity;
}

.library-instance-pull-background.visible {
	opacity: 1;
}

.library-instance-pull-handle-target {
	position: fixed;
	top: calc(var(--top-bar-height, 3rem) - 0.75rem);
	left: var(--library-instance-pull-center-x);
	z-index: 46;
	display: block;
	width: var(--library-instance-notch-hit-width);
	height: 3.75rem;
	margin: 0;
	padding: 0;
	border: 0;
	background: transparent;
	cursor: default;
	touch-action: none;
	pointer-events: auto;
	-webkit-app-region: no-drag;
}

.library-instance-pull-handle-target:active {
	cursor: default;
}

.library-instance-pull-notch {
	position: absolute;
	top: 0;
	left: 50%;
	z-index: 8;
	display: flex;
	width: var(--library-instance-notch-width);
	height: 1.5rem;
	align-items: center;
	justify-content: center;
	border: 1px solid var(--surface-5);
	border-top: 0;
	border-radius: 0 0 calc(var(--radius-lg) * 0.9) calc(var(--radius-lg) * 0.9);
	background: color-mix(in srgb, var(--surface-4) 86%, var(--surface-5));
	box-shadow:
		inset 0 -1px 0 color-mix(in srgb, var(--color-contrast) 12%, transparent),
		0 0.35rem 1.2rem color-mix(in srgb, var(--color-bg) 52%, transparent);
	opacity: var(--library-instance-notch-opacity, 0);
	pointer-events: none;
	transform: translateX(-50%);
	transform-origin: top center;
	transition:
		opacity 140ms ease,
		background 140ms ease;
}

.library-instance-pull-notch::before,
.library-instance-pull-notch::after {
	content: '';
	position: absolute;
	top: -1px;
	width: calc(var(--radius-lg) * 0.8);
	height: calc(var(--radius-lg) * 0.8);
	border-top: 1px solid var(--surface-5);
	background: radial-gradient(
		circle at bottom right,
		transparent 0,
		transparent calc(var(--radius-lg) * 0.78),
		var(--surface-5) calc(var(--radius-lg) * 0.82),
		color-mix(in srgb, var(--surface-4) 86%, var(--surface-5)) calc(var(--radius-lg) * 0.9)
	);
}

.library-instance-pull-notch::before {
	left: calc(var(--radius-lg) * -0.8);
	transform: scaleX(-1);
}

.library-instance-pull-notch::after {
	right: calc(var(--radius-lg) * -0.8);
}

.library-instance-pull-handle {
	width: var(--library-instance-handle-width);
	max-width: calc(var(--library-instance-notch-width) - 2.25rem);
	min-width: 4rem;
	height: 0.3rem;
	border-radius: 999px;
	background: color-mix(in srgb, var(--surface-5) 88%, var(--color-contrast) 12%);
	box-shadow: 0 0 0 1px color-mix(in srgb, var(--color-contrast) 36%, transparent);
	opacity: calc(0.82 + var(--library-instance-notch-hover, 0) * 0.18);
	transform: scaleY(calc(1 + var(--library-instance-notch-hover, 0) * 0.18));
	transform-origin: center;
	transition:
		opacity 120ms ease,
		transform 120ms cubic-bezier(0.2, 0.8, 0.2, 1),
		background 120ms ease;
}

.library-instance-restore-drag-target {
	position: fixed;
	left: calc(var(--library-instance-pull-center-x) - var(--library-instance-notch-hit-width) / 2);
	bottom: 0;
	z-index: 47;
	display: block;
	width: var(--library-instance-notch-hit-width);
	height: calc(var(--library-instance-notch-exposure) + 1.25rem);
	margin: 0;
	padding: 0;
	border: 0;
	background: transparent;
	cursor: default;
	touch-action: none;
	-webkit-app-region: no-drag;
}

.library-instance-restore-drag-target:active {
	cursor: default;
}

.library-instance-docked-swipe {
	position: fixed;
	left: var(--left-bar-width, 4rem);
	right: var(--library-instance-pull-right-offset);
	top: var(--top-bar-height, 3rem);
	z-index: 45;
	height: calc(100vh - var(--top-bar-height, 3rem));
	min-width: 0;
	border-radius: var(--library-instance-docked-radius, var(--radius-xl) var(--radius-xl) 0 0);
	will-change: opacity, transform;
}

.library-instance-docked-swipe:not(.library-instance-docked-swipe--restoring) {
	clip-path: inset(0 0 calc(100% - var(--library-instance-notch-exposure)) 0);
}

.library-instance-docked-frame {
	position: relative;
	box-sizing: border-box;
	height: calc(100vh - var(--top-bar-height, 3rem));
	min-width: 0;
	border-radius: var(--library-instance-docked-radius, var(--radius-xl) var(--radius-xl) 0 0);
	background: var(--color-bg);
	box-shadow:
		inset 0 0 0 1px var(--library-instance-docked-outline-color, var(--surface-5)),
		0 -0.5rem 1.75rem var(--library-instance-docked-shadow-color, var(--color-bg));
	pointer-events: none;
	overflow: hidden;
}

.library-instance-docked-content {
	height: 100%;
	overflow: hidden;
	scrollbar-gutter: stable both-edges;
}

.library-instance-docked-content > div {
	min-height: 100%;
	height: 100%;
}

.library-instance-notch-tuner {
	width: 100%;
	box-sizing: border-box;
	padding: 0.75rem;
	border: 1px solid var(--surface-5);
	border-radius: var(--radius-lg);
	background: var(--surface-3);
	box-shadow: 0 0.75rem 2rem color-mix(in srgb, var(--color-bg) 58%, transparent);
	pointer-events: auto;
	-webkit-app-region: no-drag;
}

.library-instance-notch-tuner-header {
	display: flex;
	align-items: center;
	justify-content: space-between;
	gap: 1rem;
	margin-bottom: 0.5rem;
	color: var(--color-contrast);
	font-size: 0.875rem;
	font-weight: 600;
	line-height: 1.25rem;
}

.library-instance-notch-tuner-header span:last-child {
	color: var(--color-secondary);
	font-weight: 600;
}

.library-instance-notch-tuner-control {
	display: flex;
	flex-direction: column;
	gap: 0.5rem;
	margin-top: 0.75rem;
}

.library-instance-notch-tuner-control label {
	color: var(--color-secondary);
	font-size: 0.8125rem;
	font-weight: 600;
	line-height: 1.125rem;
}

.library-instance-snap-select {
	width: 100%;
}

@media (prefers-reduced-motion: reduce) {
	.library-instance-pull-frame,
	.library-instance-docked-swipe,
	.library-instance-docked-frame,
	.library-instance-pull-handle {
		transition: none !important;
	}
}
</style>
