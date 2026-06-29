<script setup lang="ts">
import { computed } from 'vue'

import type { GhostMediaKind, GhostShape } from './ghost'
import GhostBlock from './GhostBlock.vue'

defineOptions({
	inheritAttrs: false,
})

const props = withDefaults(
	defineProps<{
		kind?: GhostMediaKind
		animated?: boolean
	}>(),
	{
		kind: 'rounded',
		animated: true,
	},
)

const shape = computed<GhostShape>(() => {
	switch (props.kind) {
		case 'circle':
			return 'circle'
		case 'square':
			return 'square'
		default:
			return 'surface'
	}
})
</script>

<template>
	<GhostBlock
		v-bind="$attrs"
		:shape="shape"
		:animated="props.animated"
		class="ghost-media"
		:class="`ghost-media--${props.kind}`"
	/>
</template>

<style scoped>
.ghost-media {
	width: 4rem;
	min-width: 0;
}

.ghost-media--square,
.ghost-media--rounded,
.ghost-media--circle {
	aspect-ratio: 1;
}

.ghost-media--banner {
	width: 100%;
	aspect-ratio: 3 / 1;
}
</style>
