<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import CommentBubble from './CommentBubble.vue'

export interface PickDetail {
	component: string
	file: string
	parent: { name: string; file: string } | null
	rect: { top: number; left: number; width: number; height: number }
	html: string
	cssClasses: string[]
}

const picked = ref<PickDetail | null>(null)

function onPick(e: Event) {
	picked.value = (e as CustomEvent<PickDetail>).detail
}

function onClose() {
	picked.value = null
}

onMounted(() => window.addEventListener('design-inspector:pick', onPick))
onUnmounted(() => window.removeEventListener('design-inspector:pick', onPick))
</script>

<template>
	<CommentBubble v-if="picked" :detail="picked" @close="onClose" />
</template>
