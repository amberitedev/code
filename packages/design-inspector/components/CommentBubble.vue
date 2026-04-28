<script setup lang="ts">
import { nextTick, onMounted, ref } from 'vue'
import { useCommentSubmit } from '../composables/useCommentSubmit'
import { usePosition } from '../composables/usePosition'
import type { PickDetail } from './DesignInspectorRoot.vue'

const props = defineProps<{ detail: PickDetail }>()
const emit = defineEmits<{ close: [] }>()

const inputRef = ref<HTMLElement | null>(null)
const commentText = ref('')
const { style } = usePosition(props.detail.rect)
const { submit, loading, error } = useCommentSubmit()

async function onSubmit() {
	const text = commentText.value.trim()
	if (!text) return
	const ok = await submit({
		id: crypto.randomUUID(),
		component: props.detail.component,
		source_file: props.detail.file,
		html: props.detail.html,
		css_classes: props.detail.cssClasses,
		parent: props.detail.parent
			? { component: props.detail.parent.name, file: props.detail.parent.file }
			: undefined,
		comment: text,
		timestamp: new Date().toISOString(),
	})
	if (ok) emit('close')
}

function onInput(e: Event) {
	commentText.value = (e.target as HTMLElement).innerText
}

function onKeydown(e: KeyboardEvent) {
	if (e.key === 'Enter' && !e.shiftKey) {
		e.preventDefault()
		onSubmit()
	}
	if (e.key === 'Escape') emit('close')
}

onMounted(async () => {
	await nextTick()
	inputRef.value?.focus()
})
</script>

<template>
	<div
		:style="[{ position: 'fixed', zIndex: '9999', minWidth: '220px', maxWidth: '400px' }, style]"
	>
		<div class="bg-surface-3 border border-divider rounded-xl p-3 shadow-lg">
			<!-- Header: component chip + close -->
			<div class="flex items-center gap-2 mb-2">
				<span
					class="text-xs font-medium px-2 py-0.5 rounded-full bg-green-500/20 text-green-400 border border-green-500/30 font-mono truncate max-w-[200px]"
				>
					{{ detail.component }}
				</span>
				<button
					class="ml-auto shrink-0 text-contrast/40 hover:text-contrast/80 leading-none text-base transition-colors"
					@click="$emit('close')"
				>
					✕
				</button>
			</div>

			<!-- Comment input -->
			<div
				ref="inputRef"
				contenteditable="true"
				class="w-full text-sm text-contrast bg-transparent outline-none"
				style="
					white-space: pre-wrap;
					word-break: break-word;
					display: block;
					min-height: 1.5em;
					max-height: 160px;
					overflow-y: auto;
				"
				data-placeholder="Add a design comment..."
				@input="onInput"
				@keydown="onKeydown"
			/>

			<!-- Footer: file path + send button -->
			<div class="flex items-center justify-between mt-2 pt-2 border-t border-divider">
				<span class="text-xs text-contrast/40 truncate max-w-[55%] font-mono">
					{{ detail.file.split('/').slice(-2).join('/') || detail.file }}
				</span>
				<button
					:disabled="loading || !commentText.trim()"
					class="shrink-0 text-xs px-2.5 py-1 rounded-lg bg-green-500 text-white font-medium disabled:opacity-40 hover:bg-green-600 transition-colors"
					@click="onSubmit"
				>
					{{ loading ? '…' : 'Send' }}
				</button>
			</div>

			<!-- Error message -->
			<p v-if="error" class="text-xs text-red-400 mt-1.5">{{ error }}</p>
		</div>
	</div>
</template>

<style scoped>
[contenteditable]:empty::before {
	content: attr(data-placeholder);
	color: var(--color-contrast, #888);
	opacity: 0.4;
	pointer-events: none;
}
</style>
