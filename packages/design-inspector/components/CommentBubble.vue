<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref } from 'vue'
import { useComponentAutocomplete } from '../composables/useComponentAutocomplete'
import { useCommentSubmit } from '../composables/useCommentSubmit'
import { copyPayloadToClipboard } from '../composables/useClipboardMode'
import { usePosition } from '../composables/usePosition'
import { useInlinePickMode } from '../composables/useInlinePickMode'
import type { PickDetail } from './DesignInspectorRoot.vue'

const props = withDefaults(defineProps<{
	detail: PickDetail
	rect: { top: number; left: number; width: number; height: number }
	bubbleId: string
	color?: string
}>(), { color: '#22c55e' })
const emit = defineEmits<{ close: []; fetchFailed: [message: string] }>()

const inputRef = ref<HTMLElement | null>(null)
const commentText = ref('')
const flashAnim = ref<'di-flash-success' | 'di-flash-error' | ''>('')
const { style, glassStyle } = usePosition(() => props.rect)
const { submit, loading, error } = useCommentSubmit()
const { picking, start: startPick } = useInlinePickMode(inputRef, props.bubbleId)
const ac = useComponentAutocomplete(inputRef, () => {
	emit('fetchFailed', 'opencode not connected — start opencode to enable # / @ autocomplete')
})

let mo: MutationObserver | null = null

/** Walk the contenteditable DOM and build comment text with inline reference objects. */
function buildCommentWithInlineRefs(el: HTMLElement | null): string {
	if (!el) return ''
	const parts: string[] = []
	function walk(node: Node) {
		if (node.nodeType === Node.TEXT_NODE) {
			parts.push(node.textContent ?? '')
		} else if (node instanceof HTMLElement) {
			if (node.classList.contains('di-hash-token')) {
				const name = (node.textContent ?? '').replace(/^[#@]/, '')
				const sourceFile = node.dataset.componentFile
				const sourceLines = node.dataset.componentLines
					? (JSON.parse(node.dataset.componentLines) as [number, number])
					: undefined
				const props = node.dataset.componentProps
					? (JSON.parse(node.dataset.componentProps) as Record<string, unknown>)
					: undefined
				const sourceFileStr = sourceFile
					? sourceLines
						? `${sourceFile}:${sourceLines[0]}-${sourceLines[1]}`
						: sourceFile
					: undefined
				if (props) {
					// click reference: [component: Name, source file: path:L-L, component properties: {...}]
					const refParts: string[] = [`component: ${name}`]
					if (sourceFileStr) refParts.push(`source file: ${sourceFileStr}`)
					refParts.push(`component properties: ${JSON.stringify(props)}`)
					parts.push(`[${refParts.join(', ')}]`)
				} else {
					// mention reference: [name: Name, source file: path:L-L]
					const refParts: string[] = [`name: ${name}`]
					if (sourceFileStr) refParts.push(`source file: ${sourceFileStr}`)
					parts.push(`[${refParts.join(', ')}]`)
				}
			} else {
				for (const child of node.childNodes) walk(child)
			}
		}
	}
	for (const child of el.childNodes) walk(child)
	return parts.join('').trim()
}

async function onSubmit() {
	const text = buildCommentWithInlineRefs(inputRef.value)
	if (!text) return
	const { file, sourceLines } = props.detail
	const sourceFileStr = sourceLines ? `${file}:${sourceLines[0]}-${sourceLines[1]}` : file
	const payload = {
		id: crypto.randomUUID(),
		component: file,
		'source file': sourceFileStr,
		comment: text,
	}
	copyPayloadToClipboard(payload)
	const ok = await submit(payload)
	if (ok) {
		flashAnim.value = 'di-flash-success'
		setTimeout(() => emit('close'), 700)
	} else if (error.value) {
		flashAnim.value = 'di-flash-error'
		emit('fetchFailed', error.value)
		setTimeout(() => { flashAnim.value = '' }, 700)
	}
}

function onInput(e: Event) {
	commentText.value = (e.target as HTMLElement).innerText
	ac.onInput(e)
}

function onKeydown(e: KeyboardEvent) {
	if (ac.onKeyDown(e)) return
	if (e.altKey && e.metaKey) { e.preventDefault(); startPick(); return }
	if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); onSubmit() }
	if (e.key === 'Escape') emit('close')
}

onMounted(async () => {
	await nextTick()
	inputRef.value?.focus()
	if (inputRef.value) {
		mo = new MutationObserver((muts) => {
			for (const m of muts) for (const n of m.removedNodes)
				if (n instanceof HTMLElement && n.dataset.colorKey)
					window.dispatchEvent(new CustomEvent('design-inspector:token-removed', { detail: { colorKey: n.dataset.colorKey } }))
		})
		mo.observe(inputRef.value, { childList: true, subtree: true })
	}
})
onUnmounted(() => { mo?.disconnect(); mo = null })
</script>

<template>
	<div :style="[{ position: 'fixed', zIndex: '9999', minWidth: '220px', maxWidth: '360px', display: 'flex', flexDirection: 'column' }, style]">
		<!-- Glass card: overflow:hidden + maxHeight from glassStyle keep it in-viewport -->
		<div
			style="
				display: flex; flex-direction: column;
				background: rgba(15, 18, 24, 0.55);
				backdrop-filter: blur(40px) saturate(2);
				-webkit-backdrop-filter: blur(40px) saturate(2);
				border: 1px solid rgba(255, 255, 255, 0.13);
				border-radius: 12px;
				box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5), 0 2px 8px rgba(0, 0, 0, 0.25);
				font-family: Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
			"
			:style="[glassStyle, flashAnim ? { animation: `${flashAnim} 700ms ease-out forwards` } : {}, picking ? { borderColor: '#22c55e', boxShadow: '0 0 0 2px rgba(34,197,94,0.35), 0 12px 40px rgba(0,0,0,0.5)' } : {}]"
		>
		<!-- Header: fixed height, never shrinks -->
		<div style="flex-shrink: 0; display: flex; align-items: center; padding: 8px 10px 7px; gap: 8px; border-bottom: 1px solid rgba(255,255,255,0.07);">
			<div :style="{ flex: '0 1 auto', minWidth: '0', display: 'flex', alignItems: 'center', background: props.color, borderRadius: '8px', padding: '3px 10px', overflow: 'hidden' }">
				<span style="font-size: 11px; font-family: ui-monospace, monospace; color: #052e16; font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{{ detail.component }}</span>
			</div>
			<button style="flex-shrink: 0; margin-left: auto; background: none; border: none; cursor: pointer; color: rgba(255,255,255,0.45); font-size: 14px; padding: 2px 4px; line-height: 1;" @click="$emit('close')">✕</button>
		</div>

		<!-- Autocomplete dropdown: fixed height, scrolls internally -->
		<div v-if="ac.open.value" style="flex-shrink: 0; border-top: 1px solid rgba(255,255,255,0.06); padding: 4px 0; max-height: 160px; overflow-y: auto;">
			<div v-if="!ac.filtered.value.length" style="padding: 5px 10px; font-size: 12px; color: rgba(255,255,255,0.32);">No results</div>
			<div
				v-for="(opt, i) in ac.filtered.value"
				:key="opt.name"
				:style="{
					display: 'flex', alignItems: 'center', gap: '6px',
					padding: '5px 10px', cursor: 'pointer',
					background: i === ac.activeIndex.value ? 'rgba(74,222,128,0.10)' : 'transparent',
				}"
				@mousedown.prevent="ac.select(opt.name, opt.path)"
				@mouseenter="ac.activeIndex.value = i"
			>
				<span style="color: #4ade80; font-family: ui-monospace, monospace; font-size: 11px; flex-shrink: 0;">{{ ac.triggerChar.value }}</span>
				<span style="font-size: 12px; color: rgba(255,255,255,0.88); font-family: ui-monospace, monospace; white-space: nowrap; flex-shrink: 0;">{{ opt.name }}</span>
				<!-- Description with right-side gradient fade instead of file path -->
				<div style="position: relative; flex: 1; min-width: 0; overflow: hidden;">
					<span style="font-size: 10px; color: rgba(255,255,255,0.32); white-space: nowrap; display: block;">{{ opt.description ?? opt.path }}</span>
					<div style="position: absolute; top: 0; right: 0; bottom: 0; width: 20px; background: linear-gradient(to right, transparent, rgba(10,13,20,0.9));"></div>
				</div>
			</div>
		</div>

		<!-- Content: takes remaining space, scrolls vertically when text grows -->
		<div style="flex: 1; min-height: 0; overflow-y: auto; padding: 6px 12px 12px;">
			<div
				ref="inputRef"
				contenteditable="true"
				style="
					font-size: 13px;
					font-family: Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
					color: rgba(255, 255, 255, 0.88);
					background: transparent;
					outline: none;
					white-space: pre-wrap;
					word-break: break-word;
					min-height: 1.5em;
				"
				data-placeholder="Add a design comment..."
				@input="onInput"
				@keydown="onKeydown"
			/>
			<div style="display: flex; justify-content: flex-end; margin-top: 8px; padding-top: 8px; border-top: 1px solid rgba(255,255,255,0.07);">
				<button
					:disabled="loading || !commentText.trim()"
					:style="{ fontSize: '11px', padding: '4px 10px', borderRadius: '8px', background: props.color, color: '#fff', fontWeight: '600', border: 'none', transition: 'opacity 0.15s' }"
					@click="onSubmit"
				>{{ loading ? '…' : 'Send' }}</button>
			</div>
		</div>
		</div>
	</div>
</template>

<style scoped>
[contenteditable]:empty::before {
	content: attr(data-placeholder);
	color: rgba(255, 255, 255, 0.28);
	pointer-events: none;
}
button:disabled { opacity: 0.35; cursor: default; }
</style>

<style>
@keyframes di-flash-success {
	0%, 15% { border-color: #eab308; box-shadow: 0 0 0 2.5px rgba(234,179,8,0.55), 0 10px 32px rgba(0,0,0,0.55); }
	100% { border-color: rgba(255,255,255,0.09); box-shadow: 0 10px 32px rgba(0,0,0,0.55), 0 4px 10px rgba(0,0,0,0.3); }
}
@keyframes di-flash-error {
	0%, 15% { border-color: #ef4444; box-shadow: 0 0 0 2.5px rgba(239,68,68,0.55), 0 10px 32px rgba(0,0,0,0.55); }
	100% { border-color: rgba(255,255,255,0.09); box-shadow: 0 10px 32px rgba(0,0,0,0.55), 0 4px 10px rgba(0,0,0,0.3); }
}
.di-hash-token { font-weight: 700; font-family: ui-monospace, monospace; }
</style>
