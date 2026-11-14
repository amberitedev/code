import { type ContractTransport, localFiles } from '@modrinth/api-client'

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

/**
 * The App's URL for a file on this computer, for an `<img>` or `<video>`. A new `revision` makes the
 * page load the file again; the App ignores its value.
 */
export function localFileUrl(path: string, revision?: string | number): string {
	return localUrl(localFiles.getLocalFile.path, { path }, revision)
}

/** The App's URL for the image at `path`, scaled to fit `size` pixels. */
export function localThumbnailUrl(path: string, size: number, revision?: string | number): string {
	return localUrl(localFiles.getThumbnail.path, { path, size: String(size) }, revision)
}

function localUrl(route: string, query: Record<string, string>, revision?: string | number) {
	if (revision !== undefined) query.revision = String(revision)
	return `${localBase}${route}?${new URLSearchParams(query)}`
}
