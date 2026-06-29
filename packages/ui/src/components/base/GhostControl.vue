<script setup lang="ts">
import { computed } from 'vue'

import type { GhostControlKind, GhostControlSize, GhostShape } from './ghost'
import GhostBlock from './GhostBlock.vue'

defineOptions({
	inheritAttrs: false,
})

const props = withDefaults(
	defineProps<{
		kind?: GhostControlKind
		size?: GhostControlSize
		animated?: boolean
	}>(),
	{
		kind: 'button',
		size: 'standard',
		animated: true,
	},
)

const shape = computed<GhostShape>(() => {
	switch (props.kind) {
		case 'chip':
		case 'pagination':
			return 'pill'
		case 'icon-button':
			return 'circle'
		default:
			return 'control'
	}
})
</script>

<template>
	<GhostBlock
		v-bind="$attrs"
		:shape="shape"
		:animated="props.animated"
		class="ghost-control"
		:class="[`ghost-control--${props.kind}`, `ghost-control--${props.size}`]"
	/>
</template>

<style scoped>
.ghost-control {
	min-width: 0;
}

.ghost-control--small {
	height: 2rem;
}

.ghost-control--standard {
	height: 2.5rem;
}

.ghost-control--large {
	height: 3rem;
}

.ghost-control--input {
	width: 100%;
	min-width: 10rem;
}

.ghost-control--select {
	width: 12rem;
	max-width: 100%;
}

.ghost-control--button {
	width: 6.5rem;
	max-width: 100%;
}

.ghost-control--icon-button {
	width: 2.5rem;
}

.ghost-control--icon-button.ghost-control--small {
	width: 2rem;
}

.ghost-control--icon-button.ghost-control--large {
	width: 3rem;
}

.ghost-control--chip {
	width: 5rem;
	max-width: 100%;
}

.ghost-control--pagination {
	width: 9rem;
	max-width: 100%;
}
</style>
