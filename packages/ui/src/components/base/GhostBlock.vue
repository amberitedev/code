<script setup lang="ts">
import { computed } from 'vue'

import { UI_GHOST_SHIMMER_MS } from '#ui/composables/ui-motion'

import type { GhostShape } from './ghost'

defineOptions({
	inheritAttrs: false,
})

const props = withDefaults(
	defineProps<{
		shape?: GhostShape
		animated?: boolean
	}>(),
	{
		shape: 'surface',
		animated: true,
	},
)

const ghostStyle = computed(() => ({
	'--ui-ghost-shimmer-ms': `${UI_GHOST_SHIMMER_MS}ms`,
}))
</script>

<template>
	<div
		v-bind="$attrs"
		class="ghost-block"
		:class="[`ghost-block--${props.shape}`, { 'ghost-block--animated': props.animated }]"
		:style="ghostStyle"
		aria-hidden="true"
	/>
</template>

<style scoped>
.ghost-block {
	position: relative;
	isolation: isolate;
	display: block;
	overflow: hidden;
	min-width: 0;
	background-color: color-mix(in srgb, var(--surface-4) 72%, var(--surface-5) 28%);
	border: 1px solid color-mix(in srgb, var(--surface-5) 72%, transparent);
	color: transparent;
	pointer-events: none;
	user-select: none;
}

.ghost-block--square {
	border-radius: 0.25rem;
}

.ghost-block--text,
.ghost-block--pill,
.ghost-block--circle {
	border-radius: 9999px;
}

.ghost-block--control,
.ghost-block--surface {
	border-radius: 0.75rem;
}

.ghost-block--circle {
	aspect-ratio: 1;
}

.ghost-block--animated {
	background-image:
		repeating-linear-gradient(
			45deg,
			transparent 0rem,
			transparent 15rem,
			color-mix(in srgb, var(--surface-5) 8%, transparent) 20rem,
			color-mix(in srgb, var(--surface-1) 18%, transparent) 25rem,
			color-mix(in srgb, var(--surface-5) 16%, transparent) 30rem,
			transparent 39rem,
			transparent 64rem
		),
		repeating-linear-gradient(
			45deg,
			transparent 0rem,
			transparent 34rem,
			color-mix(in srgb, var(--surface-1) 10%, transparent) 42rem,
			color-mix(in srgb, var(--surface-5) 12%, transparent) 50rem,
			transparent 64rem
		);
	background-attachment: fixed, fixed;
	background-position:
		0 0,
		0 0;
	background-repeat: repeat;
	background-size:
		64rem 64rem,
		64rem 64rem;
	animation: ui-ghost-page-sweep var(--ui-ghost-shimmer-ms) linear infinite;
}

@keyframes ui-ghost-page-sweep {
	to {
		background-position:
			64rem -64rem,
			64rem -64rem;
	}
}

@media (prefers-reduced-motion: reduce) {
	.ghost-block--animated {
		animation: none;
	}
}
</style>
