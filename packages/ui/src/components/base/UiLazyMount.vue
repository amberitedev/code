<script setup lang="ts">
import { computed, ref, toRef } from 'vue'

import { useLazyMount, type UiLazyMountMode } from '#ui/composables/use-lazy-mount'

const props = withDefaults(
	defineProps<{
		mode?: UiLazyMountMode
		once?: boolean
		contentKey?: unknown
		delayMs?: number
		idleTimeoutMs?: number
		rootMargin?: string
	}>(),
	{
		mode: 'visible',
		once: true,
		contentKey: undefined,
		delayMs: 0,
		idleTimeoutMs: 1000,
		rootMargin: '200px',
	},
)

const target = ref<Element | null>(null)
const { hasMounted, reset, shouldMount, trigger } = useLazyMount({
	contentKey: toRef(props, 'contentKey'),
	delayMs: toRef(props, 'delayMs'),
	idleTimeoutMs: toRef(props, 'idleTimeoutMs'),
	mode: toRef(props, 'mode'),
	once: toRef(props, 'once'),
	rootMargin: toRef(props, 'rootMargin'),
	target,
})
const slotProps = computed(() => ({
	hasMounted: hasMounted.value,
	mode: props.mode,
	mounted: shouldMount.value,
	reset,
	trigger,
}))
</script>

<template>
	<div ref="target" class="ui-lazy-mount">
		<slot v-if="shouldMount" v-bind="slotProps" />
		<slot v-else name="fallback" v-bind="slotProps" />
	</div>
</template>

<style scoped>
.ui-lazy-mount {
	min-width: 0;
}
</style>
