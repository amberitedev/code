export interface PickedComponent {
	name: string | null
	source_file: string | null
	source_lines?: [number, number]
	/** Live DOM element — available only from Win+Alt (bubble-pick) flow */
	element?: Element
	/** Parent template file (from data-v-inspector) — where this component is used */
	parentFile?: string | null
	/** Line in parentFile where this component appears */
	parentLine?: number | null
	/** Primitive prop values — identifies which of N identical instances was pointed at */
	props?: Record<string, string | number | boolean | null>
	/** Short outerHTML snapshot for visual AI context */
	html?: string
	/** Raw HTML tag name (e.g. "button") for elements without a Vue component */
	tag?: string
}

type PickCallback = (info: PickedComponent) => void
let pendingCallback: PickCallback | null = null

function handleResult(e: Event) {
	if (!pendingCallback) return
	const cb = pendingCallback
	pendingCallback = null
	cb((e as CustomEvent<PickedComponent>).detail)
}

if (typeof window !== 'undefined') {
	window.addEventListener('design-inspector:bubble-pick-result', handleResult)
}

export function requestComponentPick(callback: PickCallback): void {
	pendingCallback = callback
	window.dispatchEvent(new CustomEvent('design-inspector:start-pick-for-bubble'))
}

export function cancelComponentPick(): void {
	pendingCallback = null
}
