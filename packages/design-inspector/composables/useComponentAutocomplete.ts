import { ref, type Ref } from 'vue'
import { getColorFor } from './usePickColors'
import { getKeyStyles } from './useElementInfo'

const RELAY = '/__design-relay'

export interface ComponentOption { name: string; path: string; description?: string }

export interface TokenMeta {
	sourceFile?: string; sourceLines?: [number, number]
	parentFile?: string; parentLine?: number
	props?: Record<string, string | number | boolean | null>
	html?: string; tag?: string; colorKey?: string; color?: string; computedStyles?: Record<string, string>
}

export interface PickedRef {
	name: string; source_file: string | null; source_lines?: [number, number]
	parent_file?: string; parent_line?: number
	props?: Record<string, string | number | boolean | null>
	html?: string; tag?: string; computed_styles?: Record<string, string>
}

let componentCache: ComponentOption[] | null = null
let fileCache: ComponentOption[] | null = null
let componentFetchDone = false
let fileFetchDone = false

async function fetchList(endpoint: string): Promise<ComponentOption[] | null> {
	try {
		const res = await fetch(`${RELAY}/${endpoint}/autocomplete`)
		return res.ok ? (await res.json() as ComponentOption[]) : null
	} catch { return null }
}

/** Pre-warm both caches. Safe to call multiple times. */
export async function warmCache(): Promise<void> {
	const tasks: Promise<void>[] = []
	if (componentCache === null && !componentFetchDone) {
		componentFetchDone = true
		tasks.push(fetchList('components').then(d => { if (d) componentCache = d; else componentFetchDone = false }))
	}
	if (fileCache === null && !fileFetchDone) {
		fileFetchDone = true
		tasks.push(fetchList('files').then(d => { if (d) fileCache = d; else fileFetchDone = false }))
	}
	await Promise.allSettled(tasks)
}

/** Invalidate caches on project switch so stale lists don't linger. */
export function invalidateCache(): void {
	componentCache = null; fileCache = null; componentFetchDone = false; fileFetchDone = false
}

/** Text before cursor, skipping contenteditable=false subtrees (pick tokens). */
function getTextBeforeCursor(el: HTMLElement): string {
	const sel = window.getSelection()
	if (!sel || sel.rangeCount === 0) return ''
	const range = sel.getRangeAt(0)
	let result = ''
	const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT)
	let node: Node | null
	while ((node = walker.nextNode())) {
		// Skip text inside contenteditable=false ancestors (non-editable tokens)
		let p: Node | null = node.parentNode; let skip = false
		while (p && p !== el) {
			if ((p as Element).getAttribute?.('contenteditable') === 'false') { skip = true; break }
			p = p.parentNode
		}
		if (skip) continue
		if (node === range.startContainer) {
			result += (node as Text).textContent?.slice(0, range.startOffset) ?? ''; break
		}
		if (range.comparePoint(node, 0) <= 0) result += (node as Text).textContent ?? ''
		else break
	}
	return result
}

function getCharOffset(el: HTMLElement, node: Node, offset: number): number {
	let total = 0; const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT); let curr: Node | null
	while ((curr = walker.nextNode())) {
		if (curr === node) return total + offset
		total += (curr as Text).textContent?.length ?? 0
	}
	return total + offset
}

function setRangeAtOffset(range: Range, el: HTMLElement, charOffset: number): boolean {
	const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT); let remaining = Math.max(0, charOffset); let curr: Node | null
	while ((curr = walker.nextNode())) {
		const len = (curr as Text).textContent?.length ?? 0
		if (remaining <= len) { range.setStart(curr, remaining); return true }
		remaining -= len
	}
	return false
}

export function extractRefs(el: HTMLElement | null): PickedRef[] | undefined {
	if (!el) return undefined
	const spans = Array.from(el.querySelectorAll<HTMLElement>('.di-hash-token[data-component-file]'))
	if (!spans.length) return undefined
	return spans.map(s => ({
		name: (s.textContent ?? '').replace(/^#/, ''), source_file: s.dataset.componentFile ?? null,
		source_lines: s.dataset.componentLines ? JSON.parse(s.dataset.componentLines) as [number, number] : undefined,
		parent_file: s.dataset.parentFile || undefined, parent_line: s.dataset.parentLine ? +s.dataset.parentLine : undefined,
		props: s.dataset.componentProps ? JSON.parse(s.dataset.componentProps) : undefined,
		html: s.dataset.componentHtml || undefined, tag: s.dataset.componentTag || undefined,
		computed_styles: s.dataset.componentStyles ? JSON.parse(s.dataset.componentStyles) : undefined,
	}))
}

export function insertToken(el: HTMLElement, queryLen: number | null, name: string, trigger: string, meta?: TokenMeta) {
	const sel = window.getSelection()
	if (!sel || sel.rangeCount === 0) return
	const range = sel.getRangeAt(0); const newRange = document.createRange()
	if (queryLen !== null) {
		const from = getCharOffset(el, range.startContainer, range.startOffset) - 1 - queryLen
		if (!setRangeAtOffset(newRange, el, from)) return
		newRange.setEnd(range.startContainer, range.startOffset); newRange.deleteContents()
	} else {
		newRange.setStart(range.startContainer, range.startOffset); newRange.collapse(true)
	}
	const span = document.createElement('span')
	span.className = 'di-hash-token'; span.setAttribute('contenteditable', 'false'); span.textContent = trigger + name
	span.style.color = meta?.color ?? getColorFor(meta?.colorKey ?? meta?.sourceFile) ?? 'rgba(255,255,255,0.55)'
	if (meta?.colorKey) span.dataset.colorKey = meta.colorKey
	if (meta?.sourceFile) span.dataset.componentFile = meta.sourceFile
	if (meta?.sourceLines) span.dataset.componentLines = JSON.stringify(meta.sourceLines)
	if (meta?.parentFile) span.dataset.parentFile = meta.parentFile
	if (meta?.parentLine != null) span.dataset.parentLine = String(meta.parentLine)
	if (meta?.props) span.dataset.componentProps = JSON.stringify(meta.props)
	if (meta?.html) span.dataset.componentHtml = meta.html.slice(0, 200)
	if (meta?.tag) span.dataset.componentTag = meta.tag
	if (meta?.computedStyles) span.dataset.componentStyles = JSON.stringify(meta.computedStyles)
	const space = document.createTextNode(' '); const frag = document.createDocumentFragment()
	frag.appendChild(span); frag.appendChild(space)
	newRange.insertNode(frag); newRange.setStartAfter(space); newRange.collapse(true)
	sel.removeAllRanges(); sel.addRange(newRange)
	el.dispatchEvent(new Event('input', { bubbles: true }))
}

/** Find the first live DOM element owned by a Vue component with the given source file. */
function findLiveInstance(file: string): Element | null {
	for (const el of document.querySelectorAll('[data-v-inspector]')) {
		let n: any = el; while (n) { if (n.__vueParentComponent?.type?.__file === file) return el; n = n.parentElement }
	}
	return null
}

export function useComponentAutocomplete(inputRef: Ref<HTMLElement | null>, onServerError?: () => void) {
	const open = ref(false); const filtered = ref<ComponentOption[]>([]); const activeIndex = ref(0)
	const triggerChar = ref<'#' | '@'>('#'); let currentQuery = ''

	async function ensureCache(trigger: '#' | '@'): Promise<ComponentOption[]> {
		const isHash = trigger === '#'
		const cache = isHash ? componentCache : fileCache
		if (cache !== null) return cache
		if (isHash ? componentFetchDone : fileFetchDone) return []
		if (isHash) componentFetchDone = true; else fileFetchDone = true
		try {
			const res = await fetch(`${RELAY}/${isHash ? 'components' : 'files'}/autocomplete`)
			if (!res.ok) { onServerError?.(); return [] }
			const data = await res.json() as ComponentOption[]
			if (isHash) componentCache = data; else fileCache = data
			return data
		} catch { onServerError?.(); return [] }
	}

	function filterLocal(items: ComponentOption[], q: string): ComponentOption[] { return q ? items.filter(it => it.name.toLowerCase().includes(q.toLowerCase())).slice(0, 8) : items.slice(0, 8) }

	function onInput(_e: Event) {
		const el = inputRef.value; if (!el) return
		const text = getTextBeforeCursor(el)
		const hashMatch = text.match(/#(\S*)$/); const atMatch = text.match(/@(\S*)$/)
		const match = hashMatch ?? atMatch
		if (!match) { open.value = false; return }
		triggerChar.value = hashMatch ? '#' : '@'; currentQuery = match[1]
		open.value = true; filtered.value = []; activeIndex.value = 0
		ensureCache(triggerChar.value).then((items) => { filtered.value = filterLocal(items, currentQuery); activeIndex.value = 0 })
	}

	function onKeyDown(e: KeyboardEvent): boolean {
		if (!open.value) return false
		if (e.key === 'Escape') { open.value = false; return true }
		if (e.key === 'ArrowDown') { e.preventDefault(); if (filtered.value.length) activeIndex.value = (activeIndex.value + 1) % filtered.value.length; return true }
		if (e.key === 'ArrowUp') { e.preventDefault(); if (filtered.value.length) activeIndex.value = (activeIndex.value - 1 + filtered.value.length) % filtered.value.length; return true }
		if (e.key === 'Enter' || e.key === 'Tab') { const opt = filtered.value[activeIndex.value]; if (opt) { e.preventDefault(); select(opt.name, opt.path); return true } }
		return false
	}

	function select(name: string, path?: string) {
		const el = inputRef.value; if (!el) return
		const liveEl = path ? findLiveInstance(path) : null
		insertToken(el, currentQuery.length, name, triggerChar.value, path ? { sourceFile: path, computedStyles: liveEl ? getKeyStyles(liveEl) : undefined } : undefined)
		open.value = false; el.focus()
	}

	return { open, filtered, activeIndex, triggerChar, onInput, onKeyDown, select }
}
