// `init_script` in mod.rs wraps this in a block, so nothing leaks into the page, and calls it
// with the base URL and the main window's origins.
// eslint-disable-next-line no-unused-vars
const localInit = (base, origins) => {
	const label = window.__TAURI_INTERNALS__?.metadata?.currentWebview?.label
	if (window.top !== window || label !== 'main') return
	if (!origins.includes(location.origin)) return
	window.__AMBERITE_LOCAL__ = base
}
