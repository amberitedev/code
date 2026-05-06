<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, computed } from 'vue'
import CommentBubble from './CommentBubble.vue'
import ElementHighlight from './ElementHighlight.vue'
import FetchErrorToast from './FetchErrorToast.vue'
import InspectorClone from './InspectorClone.vue'
import InspectorOverlay from './InspectorOverlay.vue'
import { useClipboardMode } from '../composables/useClipboardMode'
import { restoreColors } from '../composables/usePickColors'
import { loadPins, savePins, findScrollContainer, findElementFor, type PinnedEntry } from '../composables/usePinnedState'
import { warmCache, invalidateCache } from '../composables/useComponentAutocomplete'

export interface PickDetail {
	component: string
	file: string
	parent: { name: string; file: string } | null
	element: Element | null
	html: string
	cssClasses: string[]
	sourceLines?: [number, number]
	computedStyles: Record<string, string>
	componentProps: Record<string, unknown>
}

interface StickyPickDetail {
	element: Element; name: string; sourceFile: string | null; sourceLines?: [number, number]
	parentFile: string | null; parentLine: number | null; props: Record<string, string | number | boolean | null> | null
	html: string; tag: string; color: string; colorKey: string; bubbleId: string
}

interface Rect { top: number; left: number; width: number; height: number }

interface Clone {
	id: string; detail: PickDetail; liveEl: Element | null
	highlightRect: Rect; bubbleRect: Rect
	scrollContainer: Element | null; prevContainerRect: Rect | null
	pathname: string; color: string
	sticky?: true; colorKey?: string; bubbleId?: string
	parentFile?: string | null; parentLine?: number | null
	propsSignature?: string
}

const clones = ref<Clone[]>([])
const toastMessage = ref<string | null>(null)
const { autoClipboard, infoMessage } = useClipboardMode()

const activeClones = computed(() => clones.value.filter(c => document.contains(c.liveEl)))

function toRect(el: Element): Rect {
	const r = el.getBoundingClientRect()
	return { top: r.top, left: r.left, width: r.width, height: r.height }
}

function persist() {
	savePins(clones.value.filter(c => c.sticky).map(c => ({
		id: c.id, pathname: c.pathname, color: c.color, colorKey: c.colorKey!,
		component: c.detail.component, sourceFile: c.detail.file, sourceLines: c.detail.sourceLines,
		parentFile: c.parentFile ?? null, parentLine: c.parentLine ?? null,
		propsSignature: c.propsSignature ?? '', html: c.detail.html,
		cssClasses: c.detail.cssClasses, parent: c.detail.parent,
		lastBubbleTop: c.bubbleRect.top, lastBubbleLeft: c.bubbleRect.left,
	} satisfies PinnedEntry)))
}

function makeClone(detail: PickDetail, color: string, el: Element): Clone {
	const hr = toRect(el); const sc = findScrollContainer(el)
	return { id: crypto.randomUUID(), detail, liveEl: el, highlightRect: { ...hr }, bubbleRect: { ...hr }, scrollContainer: sc, prevContainerRect: sc ? toRect(sc) : null, pathname: location.pathname, color }
}

/** Plain Alt click — always green, always creates a CommentBubble. */
function onPick(e: Event) {
	const detail = (e as CustomEvent<PickDetail>).detail
	if (!detail.element || clones.value.some(c => !c.sticky && c.liveEl === detail.element)) return
	clones.value.push(makeClone(detail, '#22c55e', detail.element))
}

function onStickyPick(e: Event) {
	const d = (e as CustomEvent<StickyPickDetail>).detail
	if (!d.element) return
	if (clones.value.some(c => c.sticky && c.liveEl === d.element)) return
	const ps = d.props ? JSON.stringify(Object.fromEntries(Object.entries(d.props).sort(([a],[b]) => a.localeCompare(b)))) : ''
	const hr = toRect(d.element); const sc = findScrollContainer(d.element)
	clones.value.push({
		id: crypto.randomUUID(),
		detail: { component: d.name, file: d.sourceFile ?? '', element: d.element, html: d.html, cssClasses: Array.from(d.element.classList), sourceLines: d.sourceLines, computedStyles: {}, componentProps: {}, parent: null },
		liveEl: d.element, highlightRect: { ...hr }, bubbleRect: { ...hr },
		scrollContainer: sc, prevContainerRect: sc ? toRect(sc) : null,
		pathname: location.pathname, color: d.color, sticky: true, colorKey: d.colorKey, bubbleId: d.bubbleId,
		parentFile: d.parentFile, parentLine: d.parentLine, propsSignature: ps,
	} as Clone)
	persist()
}

function onClose(id: string) {
	clones.value = clones.value.filter(c => c.id !== id && !(c.sticky && c.bubbleId === id)); persist()
}
function onFetchFailed(message: string) { toastMessage.value = message }
function dismissToast() { toastMessage.value = null }
function onTokenRemoved(e: Event) {
	const { colorKey } = (e as CustomEvent<{ colorKey: string }>).detail
	clones.value = clones.value.filter(c => !(c.sticky && c.colorKey === colorKey)); persist()
}
function tryReattach() {
	for (const c of clones.value) {
		if (c.liveEl && document.contains(c.liveEl)) continue
		if (!c.sticky || !c.parentFile) continue
		const el = findElementFor(c.detail.file || null, c.parentFile, c.parentLine ?? null, c.propsSignature ?? '')
		if (!el) continue
		c.liveEl = el; c.scrollContainer = findScrollContainer(el); c.prevContainerRect = c.scrollContainer ? toRect(c.scrollContainer) : null
		const r = toRect(el); c.highlightRect = { ...r }
		c.bubbleRect = { top: c.bubbleRect.top || r.top, left: c.bubbleRect.left || r.left, width: r.width, height: r.height }
	}
}

function onNavigation() { invalidateCache(); warmCache(); nextTick(tryReattach) }

function updateRects() {
	for (const clone of clones.value) {
		if (!clone.liveEl) continue
		if (!document.contains(clone.liveEl)) { clone.liveEl = null; continue }
		const r = clone.liveEl.getBoundingClientRect()
		clone.highlightRect.top = r.top; clone.highlightRect.left = r.left
		clone.highlightRect.width = r.width; clone.highlightRect.height = r.height
		if (clone.scrollContainer) {
			const cr = toRect(clone.scrollContainer); const prev = clone.prevContainerRect!
			if (cr.top !== prev.top || cr.left !== prev.left) {
				clone.bubbleRect.top += cr.top - prev.top; clone.bubbleRect.left += cr.left - prev.left
			}
			clone.prevContainerRect = cr
		} else {
			clone.bubbleRect.top = r.top; clone.bubbleRect.left = r.left
			clone.bubbleRect.width = r.width; clone.bubbleRect.height = r.height
		}
	}
}

function onCloseNewest(e: Event) {
	const active = activeClones.value
	if (active.length > 0) { clones.value = clones.value.filter(c => c.id !== active[active.length - 1].id); persist(); e.preventDefault() }
}
function restoreFromStorage() {
	const pins = loadPins()
	restoreColors(pins.map(p => ({ key: p.colorKey, color: p.color })))
	clones.value = pins.map(p => {
		const el = findElementFor(p.sourceFile, p.parentFile, p.parentLine, p.propsSignature)
		const sc = el ? findScrollContainer(el) : null; const base = el ? toRect(el) : { top: p.lastBubbleTop, left: p.lastBubbleLeft, width: 0, height: 0 }
		return { id: p.id, detail: { component: p.component, file: p.sourceFile ?? '', parent: p.parent, element: el, html: p.html, cssClasses: p.cssClasses, sourceLines: p.sourceLines, computedStyles: {}, componentProps: {} }, liveEl: el, highlightRect: { ...base }, bubbleRect: { ...base }, scrollContainer: sc, prevContainerRect: sc ? toRect(sc) : null, pathname: p.pathname, color: p.color, sticky: true, colorKey: p.colorKey, parentFile: p.parentFile, parentLine: p.parentLine, propsSignature: p.propsSignature } as Clone
	})
}
let _origPush: typeof history.pushState, _origReplace: typeof history.replaceState
let _mo: MutationObserver | null = null

onMounted(() => {
	restoreFromStorage(); warmCache()
	const on = window.addEventListener.bind(window)
	on('design-inspector:pick', onPick); on('design-inspector:sticky-pick', onStickyPick)
	on('design-inspector:close-newest', onCloseNewest); on('design-inspector:token-removed', onTokenRemoved)
	on('scroll', updateRects, { capture: true, passive: true }); on('resize', updateRects, { passive: true })
	on('popstate', onNavigation)
	_origPush = history.pushState.bind(history)
	_origReplace = history.replaceState.bind(history)
	history.pushState = (...args: Parameters<typeof history.pushState>) => { _origPush(...args); onNavigation() }
	history.replaceState = (...args: Parameters<typeof history.replaceState>) => { _origReplace(...args); onNavigation() }
	_mo = new MutationObserver(muts => { if (muts.some(m => m.removedNodes.length > 0)) updateRects() })
	_mo.observe(document.body, { childList: true, subtree: true })
})

onUnmounted(() => {
	const off = window.removeEventListener.bind(window)
	off('design-inspector:pick', onPick); off('design-inspector:sticky-pick', onStickyPick)
	off('design-inspector:close-newest', onCloseNewest); off('design-inspector:token-removed', onTokenRemoved)
	off('scroll', updateRects, { capture: true }); off('resize', updateRects); off('popstate', onNavigation)
	history.pushState = _origPush; history.replaceState = _origReplace; _mo?.disconnect()
})
</script>

<template>
	<InspectorOverlay />
	<template v-for="clone in activeClones" :key="clone.id">
		<InspectorClone v-if="clone.sticky" :top="clone.highlightRect.top" :left="clone.highlightRect.left"
			:width="clone.highlightRect.width" :height="clone.highlightRect.height"
			:name="clone.detail.component" :color="clone.color" />
		<ElementHighlight v-else :top="clone.highlightRect.top" :left="clone.highlightRect.left"
			:width="clone.highlightRect.width" :height="clone.highlightRect.height"
			:name="clone.detail.component" :color="clone.color" :animated="false" />
		<CommentBubble v-if="!clone.sticky"
			:detail="clone.detail" :rect="clone.bubbleRect" :bubble-id="clone.id" :color="clone.color"
			@close="onClose(clone.id)" @fetch-failed="onFetchFailed"
		/>
	</template>
	<FetchErrorToast v-if="toastMessage" :message="toastMessage" @dismiss="dismissToast" />
	<div v-if="infoMessage" style="position:fixed;bottom:76px;right:20px;z-index:10001;background:#1e1e24;border:1px solid #4ade80;border-radius:10px;padding:8px 12px;font-family:system-ui,sans-serif;font-size:11px;font-weight:600;color:#4ade80;box-shadow:0 8px 24px rgba(0,0,0,0.5);pointer-events:none;">{{ infoMessage }}</div>
	<div v-if="autoClipboard" style="position:fixed;top:12px;right:12px;z-index:9998;background:rgba(74,222,128,0.12);border:1px solid rgba(74,222,128,0.3);border-radius:6px;padding:3px 8px;font-family:ui-monospace,monospace;font-size:10px;color:#4ade80;pointer-events:none;letter-spacing:0.02em;">⌘ Clipboard ON</div>
</template>
