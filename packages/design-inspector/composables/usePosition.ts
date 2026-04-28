const MARGIN = 16
const BUBBLE_W = 260
const BUBBLE_H = 140

interface Rect {
	top: number
	left: number
	width: number
	height: number
}

/**
 * Calculates fixed-position coordinates for the comment bubble,
 * trying right → left → top → bottom-left corner fallback.
 * All coordinates are viewport-relative (for position: fixed).
 */
export function usePosition(rect: Rect): { style: Record<string, string> } {
	const vw = window.innerWidth
	const vh = window.innerHeight

	// Try right of element
	if (rect.left + rect.width + MARGIN + BUBBLE_W < vw) {
		return {
			style: {
				top: `${Math.min(rect.top, vh - BUBBLE_H - MARGIN)}px`,
				left: `${rect.left + rect.width + MARGIN}px`,
			},
		}
	}

	// Try left of element
	if (rect.left - MARGIN - BUBBLE_W > 0) {
		return {
			style: {
				top: `${Math.min(rect.top, vh - BUBBLE_H - MARGIN)}px`,
				left: `${rect.left - MARGIN - BUBBLE_W}px`,
			},
		}
	}

	// Try above element
	if (rect.top - MARGIN - BUBBLE_H > 0) {
		return {
			style: {
				top: `${rect.top - MARGIN - BUBBLE_H}px`,
				left: `${Math.min(Math.max(rect.left, MARGIN), vw - BUBBLE_W - MARGIN)}px`,
			},
		}
	}

	// Fallback: bottom-left corner with margin
	return {
		style: {
			top: `${vh - BUBBLE_H - MARGIN}px`,
			left: `${MARGIN}px`,
		},
	}
}
