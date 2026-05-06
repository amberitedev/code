import type { PickedComponent } from './useComponentPicker'

/**
 * Curated palette — 16 easily-distinguishable colours.
 * Green is first so the very first Win+Alt pick is immediately visible.
 */
export const PICK_COLORS: readonly string[] = [
	'#22c55e', // green      ← default
	'#3b82f6', // blue
	'#ef4444', // red
	'#f97316', // orange
	'#a855f7', // purple
	'#0ea5e9', // sky
	'#6366f1', // indigo
	'#14b8a6', // teal
	'#f43f5e', // rose
	'#eab308', // amber
	'#06b6d4', // cyan
	'#84cc16', // lime
	'#8b5cf6', // violet
	'#ec4899', // pink
	'#10b981', // emerald
	'#f59e0b', // yellow
]

/**
 * Per-element stable numeric ID stored in a WeakMap.
 * Guarantees two different DOM nodes always produce different colorKeys
 * even when they share identical source file, parent, and props.
 */
const elementIds = new WeakMap<Element, number>()
let elemCounter = 0
function getElementId(el: Element): number {
	if (!elementIds.has(el)) elementIds.set(el, ++elemCounter)
	return elementIds.get(el)!
}

/**
 * Builds the semantic identity key for a Win+Alt picked element.
 *
 * Key = `sourceFile::parentFile:parentLine::sortedPropsJSON::elemId`
 *
 * — elemId  guarantees distinct keys for distinct DOM nodes (fixes same-name collision)
 * — sourceFile  distinguishes NavButton from OtherButton
 * — parentFile:parentLine  distinguishes same component at different usage sites
 * — sortedPropsJSON  distinguishes list items with different prop values
 */
export function makeColorKey(
	info: Pick<PickedComponent, 'source_file' | 'parentFile' | 'parentLine' | 'props' | 'tag'>
		& { element?: Element | null },
): string {
	const file = info.source_file ?? info.tag ?? 'unknown'
	const parent = info.parentFile ? `${info.parentFile}:${info.parentLine ?? 0}` : ''
	const propsStr = info.props && Object.keys(info.props).length > 0
		? JSON.stringify(Object.fromEntries(Object.entries(info.props).sort(([a], [b]) => a.localeCompare(b))))
		: ''
	const elemId = info.element ? String(getElementId(info.element)) : ''
	return `${file}::${parent}::${propsStr}::${elemId}`
}

// Full colour registry: colorKey → colour
const registry = new Map<string, string>()
let counter = 0

/** Assign the next palette colour for this element instance. Idempotent per key. */
export function assignColor(key: string): string {
	const hit = registry.get(key)
	if (hit) return hit
	const color = PICK_COLORS[counter % PICK_COLORS.length]
	counter++
	registry.set(key, color)
	return color
}

/** Restore colour assignments from localStorage so colours survive page refresh. */
export function restoreColors(entries: Array<{ key: string; color: string }>) {
	for (const e of entries) if (!registry.has(e.key)) registry.set(e.key, e.color)
}

/**
 * Look up a colour for a token being inserted.
 * Tries exact key first; falls back to first entry whose key starts with
 * `fileOrKey + '::'` so `#NavButton` (file-path only) still gets coloured.
 */
export function getColorFor(fileOrKey: string | undefined): string | undefined {
	if (!fileOrKey) return undefined
	const exact = registry.get(fileOrKey)
	if (exact) return exact
	for (const [k, color] of registry) {
		if (k.startsWith(fileOrKey + '::')) return color
	}
	return undefined
}

/** Override the colour for an existing registry entry (used by the color picker UI). */
export function setColor(key: string, color: string): void {
	registry.set(key, color)
}
