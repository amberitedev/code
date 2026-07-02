<template>
	<div
		ref="viewport"
		class="stage-content-transition"
		:class="{ 'stage-content-transition--active': transitionActive }"
		:style="viewportStyle"
	>
		<Transition
			name="stage-content-transition-panel"
			:duration="durationMs"
			@after-enter="finishPanelTransition"
			@after-leave="finishPanelTransition"
			@enter-cancelled="finishPanelTransition"
			@leave-cancelled="finishPanelTransition"
		>
			<div
				:key="contentKey"
				class="stage-content-transition-panel"
				:data-content-key="contentKeyString"
			>
				<slot />
			</div>
		</Transition>
	</div>
</template>

<script lang="ts">
export type StageContentTransitionAxis = 'horizontal' | 'vertical'
export type StageContentTransitionDirection =
	| 'forward'
	| 'backward'
	| 'left'
	| 'right'
	| 'up'
	| 'down'

const DEFAULT_STAGE_CONTENT_TRANSITION_MS = 180
const DEFAULT_STAGE_CONTENT_TRANSITION_EASING = 'cubic-bezier(0.32, 0.72, 0, 1)'
</script>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'

const props = withDefaults(
	defineProps<{
		contentKey: string | number
		activeIndex?: number
		axis?: StageContentTransitionAxis
		direction?: StageContentTransitionDirection
		durationMs?: number
		easing?: string
	}>(),
	{
		activeIndex: undefined,
		axis: 'horizontal',
		direction: undefined,
		durationMs: DEFAULT_STAGE_CONTENT_TRANSITION_MS,
		easing: DEFAULT_STAGE_CONTENT_TRANSITION_EASING,
	},
)

const viewport = ref<HTMLElement | null>(null)
const lockedHeight = ref<string | undefined>()
const transitionActive = ref(false)
const resolvedDirection = ref<StageContentTransitionDirection>(props.direction ?? 'forward')
const contentKeyString = computed(() => String(props.contentKey))

let resizeObserver: ResizeObserver | null = null
let heightFrame: number | null = null

const transitionOffsets = computed(() => {
	const direction = normalizedDirection.value

	if (direction === 'up' || direction === 'down') {
		return {
			enterX: '0',
			enterY: direction === 'down' ? '100%' : '-100%',
			leaveX: '0',
			leaveY: direction === 'down' ? '-100%' : '100%',
		}
	}

	return {
		enterX: direction === 'right' ? '100%' : '-100%',
		enterY: '0',
		leaveX: direction === 'right' ? '-100%' : '100%',
		leaveY: '0',
	}
})

const normalizedDirection = computed(() => {
	if (resolvedDirection.value === 'forward') return props.axis === 'vertical' ? 'down' : 'right'
	if (resolvedDirection.value === 'backward') return props.axis === 'vertical' ? 'up' : 'left'
	return resolvedDirection.value
})

const viewportStyle = computed(() => ({
	height: lockedHeight.value,
	'--stage-content-transition-enter-x': transitionOffsets.value.enterX,
	'--stage-content-transition-enter-y': transitionOffsets.value.enterY,
	'--stage-content-transition-leave-x': transitionOffsets.value.leaveX,
	'--stage-content-transition-leave-y': transitionOffsets.value.leaveY,
	'--stage-content-transition-ms': `${props.durationMs}ms`,
	'--stage-content-transition-easing': props.easing,
}))

function roundHeightForDevicePixels(height: number) {
	if (typeof window === 'undefined') return Math.ceil(height)

	const dpr = window.devicePixelRatio || 1
	return Math.ceil(height * dpr) / dpr
}

function getActivePanel() {
	const container = viewport.value
	if (!container) return null

	return (
		Array.from(container.querySelectorAll<HTMLElement>('.stage-content-transition-panel')).find(
			(panel) => panel.dataset.contentKey === contentKeyString.value,
		) ?? null
	)
}

function disconnectResizeObserver() {
	resizeObserver?.disconnect()
	resizeObserver = null
}

function measureActivePanelHeight() {
	const panel = getActivePanel()
	if (!panel) return

	lockedHeight.value = `${roundHeightForDevicePixels(panel.getBoundingClientRect().height)}px`
}

function cancelHeightFrame() {
	if (heightFrame === null) return

	cancelAnimationFrame(heightFrame)
	heightFrame = null
}

function finishPanelTransition() {
	measureActivePanelHeight()
	transitionActive.value = false
}

function getViewportHeight() {
	return roundHeightForDevicePixels(viewport.value?.getBoundingClientRect().height ?? 0)
}

function lockCurrentHeight() {
	const height = getViewportHeight()
	if (height <= 0) return

	lockedHeight.value = `${height}px`
	transitionActive.value = true
}

function scheduleActivePanelHeightMeasure() {
	if (typeof window === 'undefined') {
		measureActivePanelHeight()
		return
	}

	cancelHeightFrame()
	heightFrame = requestAnimationFrame(() => {
		heightFrame = null
		measureActivePanelHeight()
	})
}

function setupActivePanelResizeObserver() {
	disconnectResizeObserver()

	const panel = getActivePanel()
	if (!panel || typeof ResizeObserver === 'undefined') return

	resizeObserver = new ResizeObserver(scheduleActivePanelHeightMeasure)
	resizeObserver.observe(panel)
}

watch(
	() => props.direction,
	(direction) => {
		if (direction) resolvedDirection.value = direction
	},
)

watch(
	() => props.activeIndex,
	(nextIndex, previousIndex) => {
		if (props.direction) return
		if (nextIndex === undefined || previousIndex === undefined || nextIndex === previousIndex)
			return

		resolvedDirection.value = nextIndex > previousIndex ? 'forward' : 'backward'
	},
)

watch(
	() => props.contentKey,
	() => {
		lockCurrentHeight()

		void nextTick(() => {
			setupActivePanelResizeObserver()
			scheduleActivePanelHeightMeasure()
		})
	},
)

onMounted(() => {
	void nextTick(() => {
		setupActivePanelResizeObserver()
		scheduleActivePanelHeightMeasure()
	})

	if (typeof window !== 'undefined') {
		window.addEventListener('resize', scheduleActivePanelHeightMeasure)
	}

	if (typeof document !== 'undefined' && document.fonts) {
		void document.fonts.ready.then(scheduleActivePanelHeightMeasure).catch(() => undefined)
	}
})

onUnmounted(() => {
	disconnectResizeObserver()
	cancelHeightFrame()

	if (typeof window !== 'undefined') {
		window.removeEventListener('resize', scheduleActivePanelHeightMeasure)
	}
})
</script>

<style scoped>
.stage-content-transition {
	display: flow-root;
	position: relative;
	width: 100%;
	transition: height var(--stage-content-transition-ms) var(--stage-content-transition-easing);
}

.stage-content-transition--active {
	overflow: hidden;
}

.stage-content-transition-panel {
	width: 100%;
	min-width: 0;
}

.stage-content-transition-panel-enter-active,
.stage-content-transition-panel-leave-active {
	transition: transform var(--stage-content-transition-ms) var(--stage-content-transition-easing);
	will-change: transform;
}

.stage-content-transition-panel-leave-active {
	position: absolute;
	top: 0;
	left: 0;
	right: 0;
	pointer-events: none;
}

.stage-content-transition-panel-enter-from {
	transform: translate3d(
		var(--stage-content-transition-enter-x),
		var(--stage-content-transition-enter-y),
		0
	);
}

.stage-content-transition-panel-leave-to {
	transform: translate3d(
		var(--stage-content-transition-leave-x),
		var(--stage-content-transition-leave-y),
		0
	);
}

.stage-content-transition-panel-enter-to,
.stage-content-transition-panel-leave-from {
	transform: translate3d(0, 0, 0);
}

@media (prefers-reduced-motion: reduce) {
	.stage-content-transition,
	.stage-content-transition-panel-enter-active,
	.stage-content-transition-panel-leave-active {
		transition: none;
	}

	.stage-content-transition-panel-enter-from,
	.stage-content-transition-panel-leave-to,
	.stage-content-transition-panel-enter-to,
	.stage-content-transition-panel-leave-from {
		transform: translate3d(0, 0, 0);
	}
}
</style>
