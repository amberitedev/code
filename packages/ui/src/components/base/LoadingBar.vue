<script setup lang="ts">
import { computed } from 'vue'

import { injectLoadingState } from '#ui/providers/loading-state'

const props = withDefaults(
	defineProps<{
		/** Bar height in pixels. */
		height?: number
		/** Background gradient. Defaults to the brand green. */
		color?: string
		/** Total bar fill duration in ms (visual progress easing). */
		duration?: number
		/** Delay in ms before the bar becomes visible after a load begins. */
		throttle?: number
		/** CSS position. Use `absolute` when wrapping in a custom positioned container (e.g. desktop top-bar offset). */
		position?: 'fixed' | 'absolute'
		/** Top offset CSS value. */
		offsetTop?: string
		/** Left offset CSS value. */
		offsetLeft?: string
		/** Right offset CSS value. */
		offsetRight?: string
	}>(),
	{
		height: 2,
		color: 'var(--loading-bar-gradient)',
		duration: 1000,
		throttle: 0,
		position: 'fixed',
		offsetTop: '0',
		offsetLeft: '0',
		offsetRight: '0',
	},
)

const loadingState = injectLoadingState(null)

// The fill and the show delay run as a CSS animation on transform and opacity, so the bar costs
// no main-thread work while a page loads. A load that ends inside `throttle` never shows it.
const active = computed(
	() => !!loadingState && loadingState.pending.value && loadingState.barEnabled.value,
)
</script>

<template>
	<div
		class="modrinth-loading-bar"
		:class="{ active }"
		:style="{
			position: props.position,
			top: props.offsetTop,
			right: props.offsetRight,
			left: props.offsetLeft,
			height: `${props.height}px`,
			borderRadius: `${props.height}px`,
			background: props.color,
			'--loading-bar-duration': `${props.duration}ms`,
			'--loading-bar-delay': `${props.throttle}ms`,
		}"
	/>
</template>

<style lang="scss" scoped>
.modrinth-loading-bar {
	z-index: 999999;
	pointer-events: none;
	transform-origin: left;
	opacity: 0;
	transition: opacity 0.4s;

	// Dropping the animation snaps the bar to full width, so a finished load fades out complete.
	&.active {
		opacity: 1;
		transition: opacity 0.1s linear var(--loading-bar-delay);
		animation: modrinth-loading-bar-fill var(--loading-bar-duration) ease-out
			var(--loading-bar-delay) both;
	}

	&::before {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		width: 100%;
		background-image: radial-gradient(80% 100% at 20% 0%, var(--color-brand) 0%, transparent 80%);
		opacity: 0.1;
		z-index: -1;
		pointer-events: none;
	}
}

@keyframes modrinth-loading-bar-fill {
	from {
		transform: scaleX(0);
	}
}
</style>
