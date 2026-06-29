<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'

import {
	clearBrowseDomFreezeFrameSnapshot,
	mountBrowseDomFreezeFrameSnapshot,
	type BrowseDomFreezeFrameSnapshot,
} from './browse-freeze-frame'

const props = defineProps<{
	snapshot: BrowseDomFreezeFrameSnapshot
}>()

const host = ref<HTMLElement | null>(null)

function renderSnapshot() {
	if (!host.value) return

	mountBrowseDomFreezeFrameSnapshot(host.value, props.snapshot)
}

watch(() => props.snapshot, renderSnapshot, { flush: 'post', immediate: true })
watch(host, renderSnapshot, { flush: 'post' })

onBeforeUnmount(() => {
	clearBrowseDomFreezeFrameSnapshot(host.value)
})
</script>

<template>
	<div ref="host" class="app-browse-freeze-frame" aria-hidden="true" inert></div>
</template>

<style scoped>
.app-browse-freeze-frame {
	position: fixed;
	inset: 0;
	pointer-events: none;
}
</style>
