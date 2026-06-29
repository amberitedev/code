<script setup lang="ts">
/**
 * If `pending` is false on mount and never becomes true, the slot renders with no
 * enter transition (cache-hit fast path). After a real pending phase, transitions
 * behave as before for subsequent toggles.
 */
import type { Ref } from 'vue'
import { computed, onBeforeUnmount, ref, toRef, useSlots, watch } from 'vue'

import { injectLoadingState } from '#ui/providers/loading-state'

import type { ReadyTransitionSlotProps, ReadyTransitionState } from './ready-transition'

const props = withDefaults(
	defineProps<{
		/** True while the wrapped content is still loading. Slot stays blank, loading bar runs. */
		pending: boolean | Ref<boolean>
		/** Fade duration applied to the slot when content reveals. */
		duration?: number
		/** When true, do NOT register a token with the global loading bar - only fade locally. */
		silent?: boolean
		/** Key for the content currently being resolved. Enables keyed previous-content retention. */
		contentKey?: string | number
		/** Error object for this boundary. Any non-nullish value wins over pending and timeout. */
		error?: unknown
		/** Delay before the ghost slot is shown. */
		ghostDelayMs?: number
		/** Minimum time the ghost slot stays visible after it appears. */
		minimumGhostMs?: number
		/** Optional timeout that releases this boundary's loading token and shows the timeout slot. */
		timeoutMs?: number
		/** Key used to restart timeout ownership. Defaults to contentKey. */
		timeoutKey?: string | number
		/** Keep the previous keyed content visible during the next pending phase. */
		keepPrevious?: boolean
	}>(),
	{
		duration: 200,
		silent: false,
		ghostDelayMs: 120,
		minimumGhostMs: 180,
		keepPrevious: false,
	},
)

const slots = useSlots()
const pendingRef = toRef(props, 'pending') as Ref<boolean | Ref<boolean>>
const resolvedPending = computed(() => {
	const v = pendingRef.value
	if (typeof v === 'boolean') return v
	return Boolean((v as Ref<boolean>).value)
})
const hasError = computed(() => props.error !== undefined && props.error !== null)
const currentTimeoutKey = computed(() => props.timeoutKey ?? props.contentKey)
const hasContentKey = computed(() => props.contentKey !== undefined && props.contentKey !== null)

const loadingState = injectLoadingState(null)
const hasBeenPending = ref(false)
const hasResolvedContent = ref(!resolvedPending.value && !hasError.value)
const displayedContentKey = ref<string | number | undefined>(props.contentKey)
const state = ref<ReadyTransitionState>(
	hasError.value ? 'error' : resolvedPending.value ? 'pending-hidden' : 'idle',
)
const timedOut = ref(false)

let token: symbol | null = null
let ghostTimer: ReturnType<typeof setTimeout> | undefined
let timeoutTimer: ReturnType<typeof setTimeout> | undefined
let minimumGhostTimer: ReturnType<typeof setTimeout> | undefined
let ghostShownAt = 0
let pendingCycleActive = false
let lastContentKey = props.contentKey
let lastTimeoutKey = currentTimeoutKey.value

function nowMs() {
	if (typeof performance !== 'undefined') return performance.now()

	return Date.now()
}

function normalizedMs(value: number | undefined, fallback = 0) {
	if (value === undefined || !Number.isFinite(value)) return fallback

	return Math.max(0, value)
}

function release() {
	if (token && loadingState) {
		loadingState.end(token)
	}
	token = null
}

function syncLoadingToken() {
	if (
		loadingState &&
		!props.silent &&
		typeof window !== 'undefined' &&
		resolvedPending.value &&
		!hasError.value &&
		!timedOut.value
	) {
		if (!token) token = loadingState.begin()
		return
	}

	release()
}

function clearGhostTimer() {
	if (!ghostTimer) return

	clearTimeout(ghostTimer)
	ghostTimer = undefined
}

function clearTimeoutTimer() {
	if (!timeoutTimer) return

	clearTimeout(timeoutTimer)
	timeoutTimer = undefined
}

function clearMinimumGhostTimer() {
	if (!minimumGhostTimer) return

	clearTimeout(minimumGhostTimer)
	minimumGhostTimer = undefined
}

function clearTimers() {
	clearGhostTimer()
	clearTimeoutTimer()
	clearMinimumGhostTimer()
}

function shouldKeepPreviousContent() {
	return props.keepPrevious && hasContentKey.value && hasResolvedContent.value
}

function finishResolvedContent() {
	clearTimers()
	release()
	pendingCycleActive = false
	timedOut.value = false
	ghostShownAt = 0
	displayedContentKey.value = props.contentKey
	hasResolvedContent.value = true
	state.value = hasBeenPending.value ? 'resolved' : 'idle'
}

function finishAfterMinimumGhost() {
	const minimumGhostMs = normalizedMs(props.minimumGhostMs)
	const remainingMs = minimumGhostMs - (nowMs() - ghostShownAt)
	if (state.value === 'ghost-visible' && !shouldKeepPreviousContent() && remainingMs > 0) {
		clearMinimumGhostTimer()
		minimumGhostTimer = setTimeout(finishResolvedContent, remainingMs)
		return
	}

	finishResolvedContent()
}

function showGhost() {
	ghostTimer = undefined
	if (!resolvedPending.value || hasError.value || timedOut.value) return

	ghostShownAt = nowMs()
	state.value = 'ghost-visible'
}

function startTimeoutTimer() {
	clearTimeoutTimer()
	if (props.timeoutMs === undefined) return

	timeoutTimer = setTimeout(() => {
		timeoutTimer = undefined
		if (!resolvedPending.value || hasError.value) return

		clearGhostTimer()
		clearMinimumGhostTimer()
		release()
		pendingCycleActive = false
		timedOut.value = true
		state.value = 'timeout'
	}, normalizedMs(props.timeoutMs))
}

function startPendingCycle() {
	clearTimers()
	release()
	hasBeenPending.value = true
	pendingCycleActive = true
	timedOut.value = false
	ghostShownAt = 0
	state.value = 'pending-hidden'
	syncLoadingToken()

	const ghostDelayMs = normalizedMs(props.ghostDelayMs)
	if (ghostDelayMs === 0) {
		showGhost()
	} else {
		ghostTimer = setTimeout(showGhost, ghostDelayMs)
	}

	startTimeoutTimer()
}

function syncBoundary() {
	const contentKeyChanged = !Object.is(lastContentKey, props.contentKey)
	const timeoutKeyChanged = !Object.is(lastTimeoutKey, currentTimeoutKey.value)

	if (contentKeyChanged || timeoutKeyChanged) {
		clearTimers()
		release()
		pendingCycleActive = false
		timedOut.value = false
		ghostShownAt = 0
		lastContentKey = props.contentKey
		lastTimeoutKey = currentTimeoutKey.value
	}

	if (hasError.value) {
		clearTimers()
		release()
		pendingCycleActive = false
		state.value = 'error'
		return
	}

	if (resolvedPending.value) {
		if (timedOut.value) {
			state.value = 'timeout'
			syncLoadingToken()
			return
		}
		if (!pendingCycleActive) {
			startPendingCycle()
			return
		}

		syncLoadingToken()
		return
	}

	finishAfterMinimumGhost()
}

watch(
	[
		resolvedPending,
		hasError,
		() => props.silent,
		() => props.contentKey,
		currentTimeoutKey,
		() => props.timeoutMs,
	],
	syncBoundary,
	{ immediate: true },
)

onBeforeUnmount(() => {
	clearTimers()
	release()
})

const slotProps = computed<ReadyTransitionSlotProps>(() => ({
	state: state.value,
	timedOut: timedOut.value,
	error: props.error,
}))
const canKeepPreviousContent = computed(shouldKeepPreviousContent)
const renderKind = computed<'content' | 'ghost' | 'timeout' | 'error' | 'empty'>(() => {
	if (state.value === 'error') return slots.error ? 'error' : 'empty'

	if (state.value === 'timeout') {
		if (slots.timeout) return 'timeout'
		if (canKeepPreviousContent.value) return 'content'

		return 'empty'
	}

	if (state.value === 'ghost-visible') {
		if (canKeepPreviousContent.value) return 'content'
		if (slots.ghost) return 'ghost'

		return 'empty'
	}

	if (resolvedPending.value) {
		if (canKeepPreviousContent.value) return 'content'
		if (state.value === 'ghost-visible' && slots.ghost) return 'ghost'

		return 'empty'
	}

	return 'content'
})
const useShell = computed(() => state.value !== 'idle' || renderKind.value !== 'content')
const renderedContentKey = computed(() =>
	hasContentKey.value ? `content:${String(displayedContentKey.value)}` : 'content',
)
</script>

<template>
	<template v-if="useShell">
		<Transition name="ready-fade" mode="out-in" :duration="props.duration">
			<div
				v-if="renderKind === 'content'"
				:key="renderedContentKey"
				class="ready-transition-content"
			>
				<slot v-bind="slotProps" />
			</div>
			<div v-else-if="renderKind === 'ghost'" key="ghost" class="ready-transition-ghost">
				<slot name="ghost" v-bind="slotProps" />
			</div>
			<div v-else-if="renderKind === 'timeout'" key="timeout" class="ready-transition-timeout">
				<slot name="timeout" v-bind="slotProps" />
			</div>
			<div v-else-if="renderKind === 'error'" key="error" class="ready-transition-error">
				<slot name="error" v-bind="slotProps" />
			</div>
			<div
				v-else
				key="empty"
				class="ready-transition-empty"
				aria-hidden="true"
				inert
			/>
		</Transition>
	</template>
	<slot v-else v-bind="slotProps" />
</template>

<style scoped>
.ready-fade-enter-active,
.ready-fade-leave-active {
	transition: opacity v-bind('`${props.duration}ms`') ease-in-out;
}

.ready-fade-enter-from,
.ready-fade-leave-to {
	opacity: 0;
}

.ready-transition-content,
.ready-transition-ghost,
.ready-transition-timeout,
.ready-transition-error,
.ready-transition-empty {
	width: 100%;
}

.ready-transition-empty {
	height: 100%;
	min-height: 1px;
	pointer-events: none;
}

@media (prefers-reduced-motion: reduce) {
	.ready-fade-enter-active,
	.ready-fade-leave-active {
		transition: none;
	}
}
</style>
