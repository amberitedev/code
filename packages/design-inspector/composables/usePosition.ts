import { computed } from 'vue'

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
 * Calculates fixed-position coordinates for the comment bubble.
 * Returns `style` (top/left for the outer wrapper) and `glassStyle`
 * (maxHeight + overflow:hidden for the inner card) so the bubble never
 * clips off the bottom of the viewport — it scrolls its content instead.
 * Tries: right → left → above → top-left corner fallback.
 */
export function usePosition(getRect: () => Rect) {
	const pos = computed(() => {
		const rect = getRect()
		const vw = window.innerWidth
		const vh = window.innerHeight
		let top: number, left: number

		if (rect.left + rect.width + MARGIN + BUBBLE_W < vw) {
			top = Math.min(rect.top, vh - BUBBLE_H - MARGIN)
			left = rect.left + rect.width + MARGIN
		} else if (rect.left - MARGIN - BUBBLE_W > 0) {
			top = Math.min(rect.top, vh - BUBBLE_H - MARGIN)
			left = rect.left - MARGIN - BUBBLE_W
		} else if (rect.top - MARGIN - BUBBLE_H > 0) {
			top = rect.top - MARGIN - BUBBLE_H
			left = Math.min(Math.max(rect.left, MARGIN), vw - BUBBLE_W - MARGIN)
		} else {
			top = vh - BUBBLE_H - MARGIN
			left = MARGIN
		}

		top = Math.max(MARGIN, top)
		return { top, left, maxH: vh - top - MARGIN }
	})

	const style = computed(() => ({
		top: `${pos.value.top}px`,
		left: `${pos.value.left}px`,
	}))

	/** Apply to the inner glass card so it clips/scrolls rather than overflows viewport. */
	const glassStyle = computed(() => ({
		maxHeight: `${pos.value.maxH}px`,
		overflow: 'hidden' as const,
	}))

	return { style, glassStyle }
}
