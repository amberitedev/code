<script setup lang="ts">
import { computed } from 'vue'

import { UI_GHOST_SHIMMER_MS } from '#ui/composables/ui-motion'

import NavTabs from './NavTabs.vue'

defineOptions({
	inheritAttrs: false,
})

const props = withDefaults(
	defineProps<{
		count?: number
		activeIndex?: number
		labels?: string[]
		animated?: boolean
	}>(),
	{
		count: 3,
		activeIndex: 0,
		labels: undefined,
		animated: true,
	},
)

const fallbackLabels = ['Overview', 'Content', 'Settings', 'Activity', 'Versions']
const links = computed(() => {
	const count = Math.max(2, Math.floor(props.count))

	return Array.from({ length: count }, (_, index) => ({
		label: props.labels?.[index] ?? fallbackLabels[index] ?? `Tab ${index + 1}`,
		href: `ghost-tab-${index}`,
	}))
})
const selectedIndex = computed(() => Math.min(Math.max(0, props.activeIndex), links.value.length - 1))
const ghostStyle = computed(() => ({
	'--ui-ghost-shimmer-ms': `${UI_GHOST_SHIMMER_MS}ms`,
}))
</script>

<template>
	<div
		v-bind="$attrs"
		class="ghost-tab-group"
		:class="{ 'ghost-tab-group--animated': props.animated }"
		:style="ghostStyle"
		aria-hidden="true"
		inert
	>
		<NavTabs mode="local" :active-index="selectedIndex" :links="links" />
	</div>
</template>

<style scoped>
.ghost-tab-group {
	width: fit-content;
	max-width: 100%;
	pointer-events: none;
	user-select: none;
}

.ghost-tab-group :deep(.button-animation) {
	cursor: default;
}

.ghost-tab-group :deep(.button-animation span) {
	position: relative;
	display: inline-block;
	min-width: 3.5rem;
	overflow: hidden;
	border-radius: 9999px;
	background-color: color-mix(in srgb, var(--surface-4) 72%, var(--surface-5) 28%);
	color: transparent !important;
}

.ghost-tab-group--animated :deep(.button-animation span) {
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
	animation: ui-ghost-tab-page-sweep var(--ui-ghost-shimmer-ms) linear infinite;
}

@keyframes ui-ghost-tab-page-sweep {
	to {
		background-position:
			64rem -64rem,
			64rem -64rem;
	}
}

@media (prefers-reduced-motion: reduce) {
	.ghost-tab-group--animated :deep(.button-animation span) {
		animation: none;
	}
}
</style>
