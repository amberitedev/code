<script setup lang="ts">
import { onMounted, ref } from 'vue'

const props = defineProps<{ message: string }>()
const emit = defineEmits<{ dismiss: [] }>()

const visible = ref(true)

onMounted(() => {
	setTimeout(() => {
		visible.value = false
		setTimeout(() => emit('dismiss'), 300)
	}, 4000)
})
</script>

<template>
	<Transition name="toast">
		<div
			v-if="visible"
			style="
				position: fixed;
				bottom: 20px;
				right: 20px;
				z-index: 10000;
				display: flex;
				align-items: center;
			gap: 10px;
			background: #1e1e24;
			border: 1px solid #f87171;
			border-radius: 10px;
			padding: 10px 14px;
			box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
			max-width: 360px;
			font-family: system-ui, sans-serif;
		"
	>
		<div style="flex: 1; min-width: 0;">
				<p style="font-size: 11px; font-weight: 600; color: #f87171; margin: 0 0 2px;">Design Inspector: Comment failed</p>
				<p style="font-size: 11px; color: rgba(255,255,255,0.6); margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{{ message }}</p>
			</div>
			<button
				style="background: none; border: none; cursor: pointer; color: rgba(255,255,255,0.4); font-size: 14px; padding: 0; line-height: 1; flex-shrink: 0;"
				@click="emit('dismiss')"
			>
				✕
			</button>
		</div>
	</Transition>
</template>

<style scoped>
.toast-enter-active,
.toast-leave-active {
	transition: opacity 0.2s ease, transform 0.2s ease;
}
.toast-enter-from,
.toast-leave-to {
	opacity: 0;
	transform: translateY(8px);
}
</style>
