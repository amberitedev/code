<script setup lang="ts">
import { computed } from 'vue'

import type { GhostTextKind } from './ghost'
import GhostBlock from './GhostBlock.vue'

defineOptions({
	inheritAttrs: false,
})

const props = withDefaults(
	defineProps<{
		kind?: GhostTextKind
		lines?: number
		width?: string
		lastWidth?: string
		animated?: boolean
	}>(),
	{
		kind: 'body',
		lines: 1,
		width: undefined,
		lastWidth: undefined,
		animated: true,
	},
)

const lineCount = computed(() => Math.max(1, Math.floor(props.lines)))
const defaultWidth = computed(() => {
	switch (props.kind) {
		case 'title':
			return '65%'
		case 'metadata':
			return '42%'
		default:
			return '100%'
	}
})

function getLineWidth(index: number) {
	if (index === lineCount.value - 1) {
		return props.lastWidth ?? props.width ?? (lineCount.value > 1 ? '72%' : defaultWidth.value)
	}

	return props.width ?? defaultWidth.value
}
</script>

<template>
	<div
		v-bind="$attrs"
		class="ghost-text"
		:class="`ghost-text--${props.kind}`"
		aria-hidden="true"
	>
		<GhostBlock
			v-for="index in lineCount"
			:key="index"
			shape="text"
			:animated="props.animated"
			class="ghost-text__line"
			:style="{ width: getLineWidth(index - 1) }"
		/>
	</div>
</template>

<style scoped>
.ghost-text {
	display: flex;
	width: 100%;
	min-width: 0;
	flex-direction: column;
	gap: 0.5rem;
}

.ghost-text__line {
	max-width: 100%;
}

.ghost-text--title .ghost-text__line {
	height: 1.5rem;
}

.ghost-text--body .ghost-text__line {
	height: 1rem;
}

.ghost-text--metadata .ghost-text__line {
	height: 0.75rem;
}
</style>
