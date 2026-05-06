export interface ComponentInfo {
	name: string
	file: string
	parent: { name: string; file: string } | null
	props: Record<string, unknown>
}

export function getComponentInfo(el: Element): ComponentInfo | null {
	let node: any = el
	while (node) {
		const vc = node.__vueParentComponent
		if (vc) {
			const props: Record<string, unknown> = {}
			for (const [k, v] of Object.entries((vc.props ?? {}) as Record<string, unknown>)) {
				if (v === null || v === undefined || typeof v === 'string' || typeof v === 'number' || typeof v === 'boolean') {
					props[k] = v
				} else if (typeof v === 'function') {
					props[k] = '[function]'
				} else {
					props[k] = '[object]'
				}
			}
			return {
				name: vc.type?.name || vc.type?.__name || 'Anonymous',
				file: vc.type?.__file || '',
				props,
				parent: vc.parent
					? { name: vc.parent.type?.name || vc.parent.type?.__name || '', file: vc.parent.type?.__file || '' }
					: null,
			}
		}
		node = node.parentElement
	}
	return null
}

/** Parse data-v-inspector="rel/path.vue:line:col" → { file, lines } */
export function parseInspectorInfo(el: Element): { file: string; lines: [number, number] } | undefined {
	const raw = el.getAttribute('data-v-inspector') ?? el.closest('[data-v-inspector]')?.getAttribute('data-v-inspector')
	if (!raw) return undefined
	const m = raw.match(/^(.+):(\d+):\d+$/)
	return m ? { file: m[1], lines: [+m[2], +m[2]] } : undefined
}

/** Key design-relevant computed styles — keeps payload small. */
export function getKeyStyles(el: Element): Record<string, string> {
	const s = getComputedStyle(el)
	return {
		color: s.color, background: s.backgroundColor,
		fontSize: s.fontSize, fontWeight: s.fontWeight, fontFamily: s.fontFamily,
		padding: s.padding, margin: s.margin,
		width: s.width, height: s.height,
		display: s.display, borderRadius: s.borderRadius, opacity: s.opacity,
	}
}
