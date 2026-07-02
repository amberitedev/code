export interface BrowseFreezeFrameRect {
	left: number
	top: number
	width: number
	height: number
}

export interface BrowseFreezeFrameSource {
	key: string
	element: HTMLElement
	index?: number
}

export interface BrowseDomFreezeFrameItem {
	key: string
	rect: BrowseFreezeFrameRect
	clone: HTMLElement
}

export interface BrowseDomFreezeFrameSnapshot {
	viewport: BrowseFreezeFrameRect
	items: BrowseDomFreezeFrameItem[]
}

interface MeasuredRect extends BrowseFreezeFrameRect {
	right: number
	bottom: number
}

export interface CreateBrowseDomFreezeFrameOptions {
	viewport?: HTMLElement | DOMRect | BrowseFreezeFrameRect
	overscan?: number
	maxItems?: number
}

const DEFAULT_OVERSCAN_PX = 160
const DEFAULT_MAX_ITEMS = 14

function toRect(rect: DOMRect | BrowseFreezeFrameRect): BrowseFreezeFrameRect {
	return {
		left: rect.left,
		top: rect.top,
		width: rect.width,
		height: rect.height,
	}
}

function toMeasuredRect(rect: DOMRect | BrowseFreezeFrameRect): MeasuredRect {
	const base = toRect(rect)

	return {
		...base,
		right: 'right' in rect ? rect.right : rect.left + rect.width,
		bottom: 'bottom' in rect ? rect.bottom : rect.top + rect.height,
	}
}

function getViewportRect(viewport?: HTMLElement | DOMRect | BrowseFreezeFrameRect): MeasuredRect {
	if (viewport instanceof HTMLElement) {
		return toMeasuredRect(viewport.getBoundingClientRect())
	}

	if (viewport) {
		return toMeasuredRect(viewport)
	}

	if (typeof window === 'undefined') {
		return { left: 0, top: 0, width: 0, height: 0, right: 0, bottom: 0 }
	}

	return {
		left: 0,
		top: 0,
		width: window.innerWidth,
		height: window.innerHeight,
		right: window.innerWidth,
		bottom: window.innerHeight,
	}
}

function expandRect(rect: MeasuredRect, overscan: number): MeasuredRect {
	return {
		left: rect.left - overscan,
		top: rect.top - overscan,
		width: rect.width + overscan * 2,
		height: rect.height + overscan * 2,
		right: rect.right + overscan,
		bottom: rect.bottom + overscan,
	}
}

function rectIntersectsBounds(rect: BrowseFreezeFrameRect, bounds: MeasuredRect) {
	return (
		rect.left < bounds.right &&
		rect.left + rect.width > bounds.left &&
		rect.top < bounds.bottom &&
		rect.top + rect.height > bounds.top
	)
}

function cloneFreezeFrameElement(element: HTMLElement, rect: BrowseFreezeFrameRect) {
	const clone = element.cloneNode(true) as HTMLElement

	clone.removeAttribute('id')
	clone.setAttribute('aria-hidden', 'true')
	clone.setAttribute('data-browse-freeze-frame-clone', '')
	clone.classList.add('app-browse-freeze-frame__clone')
	;(clone as HTMLElement & { inert?: boolean }).inert = true

	for (const child of clone.querySelectorAll<HTMLElement>('[id]')) {
		child.removeAttribute('id')
	}

	for (const child of clone.querySelectorAll<HTMLElement>(
		'a, button, input, select, textarea, [tabindex]',
	)) {
		child.tabIndex = -1
	}

	for (const image of clone.querySelectorAll<HTMLImageElement>('img')) {
		image.draggable = false
		image.loading = 'eager'
		image.decoding = 'async'
	}

	Object.assign(clone.style, {
		position: 'fixed',
		left: `${rect.left}px`,
		top: `${rect.top}px`,
		width: `${rect.width}px`,
		height: `${rect.height}px`,
		margin: '0',
		transform: 'none',
		transformOrigin: 'center',
		boxSizing: 'border-box',
		maxWidth: 'none',
		minWidth: '0',
		pointerEvents: 'none',
		contain: 'layout paint',
		transition: 'none',
		willChange: 'opacity, transform',
	})

	return clone
}

export function createBrowseDomFreezeFrameSnapshot(
	sources: Iterable<BrowseFreezeFrameSource>,
	options: CreateBrowseDomFreezeFrameOptions = {},
): BrowseDomFreezeFrameSnapshot | null {
	if (typeof window === 'undefined') return null

	const viewport = getViewportRect(options.viewport)
	const bounds = expandRect(viewport, options.overscan ?? DEFAULT_OVERSCAN_PX)
	const maxItems = options.maxItems ?? DEFAULT_MAX_ITEMS
	const measuredSources = Array.from(sources)
		.map((source, order) => {
			const rect = source.element.getBoundingClientRect()
			if (rect.width <= 0 || rect.height <= 0) return undefined

			return {
				...source,
				order,
				index: source.index ?? order,
				rect: toRect(rect),
			}
		})
		.filter((source): source is BrowseFreezeFrameSource & {
			order: number
			index: number
			rect: BrowseFreezeFrameRect
		} => !!source)
		.filter((source) => rectIntersectsBounds(source.rect, bounds))
		.sort((a, b) => a.index - b.index || a.order - b.order)
		.slice(0, maxItems)

	if (!measuredSources.length) return null

	return {
		viewport,
		items: measuredSources.map((source) => ({
			key: source.key,
			rect: source.rect,
			clone: cloneFreezeFrameElement(source.element, source.rect),
		})),
	}
}

export function mountBrowseDomFreezeFrameSnapshot(
	host: HTMLElement,
	snapshot: BrowseDomFreezeFrameSnapshot,
) {
	host.replaceChildren()

	for (const item of snapshot.items) {
		host.appendChild(item.clone)
	}
}

export function clearBrowseDomFreezeFrameSnapshot(host: HTMLElement | null) {
	host?.replaceChildren()
}

export function decodeImagesInElement(root: ParentNode | null | undefined, maxImages = 8) {
	if (!root) return

	for (const image of Array.from(root.querySelectorAll<HTMLImageElement>('img')).slice(
		0,
		maxImages,
	)) {
		image.loading = 'eager'
		image.decoding = 'async'

		if (typeof image.decode === 'function') {
			void image.decode().catch(() => undefined)
		}
	}
}
