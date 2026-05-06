import { getComponentInfo, parseInspectorInfo } from './useElementInfo'

export interface PinnedEntry {
	id: string
	pathname: string
	color: string
	colorKey: string
	component: string
	/** Component's own source file */
	sourceFile: string | null
	sourceLines?: [number, number]
	/** Parent template file (from data-v-inspector) — used for re-finding */
	parentFile: string | null
	/** Line in parentFile — used for re-finding */
	parentLine: number | null
	/** Sorted JSON of primitive props — disambiguates 20 identical list items */
	propsSignature: string
	html: string
	cssClasses: string[]
	parent: { name: string; file: string } | null
	lastBubbleTop: number
	lastBubbleLeft: number
}

const STORAGE_KEY = 'di:pins'

export function loadPins(): PinnedEntry[] {
	try { return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]') as PinnedEntry[] }
	catch { return [] }
}

export function savePins(pins: PinnedEntry[]): void {
	try { localStorage.setItem(STORAGE_KEY, JSON.stringify(pins)) }
	catch { /* storage full */ }
}

export function findScrollContainer(el: Element): Element | null {
	let cur = el.parentElement
	while (cur && cur !== document.body) {
		const { overflowY, overflowX } = getComputedStyle(cur)
		if (/scroll|auto/.test(overflowY) || /scroll|auto/.test(overflowX)) return cur
		cur = cur.parentElement
	}
	return null
}

/**
 * Re-find a specific element instance after SPA navigation.
 *
 * Matches by: parentFile (data-v-inspector file) + parentLine + component file.
 * Among candidates, picks the one whose props match the stored signature.
 * Falls back to first candidate if props don't match (content may have changed).
 */
export function findElementFor(
	sourceFile: string | null,
	parentFile: string | null,
	parentLine: number | null,
	propsSignature: string,
): Element | null {
	if (!parentFile || parentLine == null) return null
	let fallback: Element | null = null
	for (const el of document.querySelectorAll('[data-v-inspector]')) {
		const insp = parseInspectorInfo(el)
		if (!insp || insp.file !== parentFile || insp.lines[0] !== parentLine) continue
		// Component file check (if we have it)
		if (sourceFile) {
			const comp = getComponentInfo(el)
			if (comp && comp.file && comp.file !== sourceFile) continue
		}
		if (!fallback) fallback = el
		// Match by props for exact instance identification
		if (propsSignature) {
			const comp = getComponentInfo(el)
			const elProps = comp?.props && Object.keys(comp.props).length > 0
				? JSON.stringify(Object.fromEntries(Object.entries(comp.props).sort(([a], [b]) => a.localeCompare(b))))
				: ''
			if (elProps === propsSignature) return el
		} else {
			return el
		}
	}
	return fallback
}
