import type { ContractTransport } from '@modrinth/api-client'

declare global {
	interface Window {
		/** The App's init script sets it in the native window only. */
		__AMBERITE_LOCAL__?: string
	}
}

const STORAGE_KEY = 'amberite:local'

/**
 * A tab gets the base in its `#local=` fragment, which is taken off the address bar and kept for
 * reloads in `sessionStorage`.
 */
function readLocalBase(): string | undefined {
	const injected = window.__AMBERITE_LOCAL__
	if (injected) return injected
	const fragment = new URLSearchParams(location.hash.slice(1)).get('local')
	if (fragment) {
		history.replaceState(history.state, '', location.pathname + location.search)
		try {
			sessionStorage.setItem(STORAGE_KEY, fragment)
		} catch {}
		return fragment
	}
	let stored: string | null = null
	try {
		stored = sessionStorage.getItem(STORAGE_KEY)
	} catch {}
	return stored || undefined
}

/**
 * `http://127.0.0.1:<port>/<secret>`: the App's local server. Empty when the page was given none,
 * which `loadBoot` reports.
 */
export const localBase = readLocalBase() ?? ''

export const isNativeWindow = window.__AMBERITE_LOCAL__ !== undefined

export const localTransport: ContractTransport = {
	base: localBase,
	headers: async () => ({}),
	fetch: (input, init) => fetch(input, init),
}
