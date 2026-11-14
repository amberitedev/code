;(() => {
	const internals = window.__TAURI_INTERNALS__
	const label = internals?.metadata?.currentWebview?.label
	if (window.top !== window || label !== 'main') return

	function serialize(value) {
		try {
			if (value === null || typeof value !== 'object') return String(value)
			const seen = new WeakSet()
			return (
				JSON.stringify(value, (_, item) => {
					if (item instanceof Error) return item.stack || item.message
					if (typeof Element !== 'undefined' && item instanceof Element) return item.outerHTML
					if (['bigint', 'undefined', 'function', 'symbol'].includes(typeof item)) {
						return String(item)
					}
					if (item && typeof item === 'object') {
						if (seen.has(item)) return '[Circular]'
						seen.add(item)
					}
					return item
				}) ?? String(value)
			)
		} catch {
			return '[Unserializable value]'
		}
	}

	// One request at a time, carrying every line logged meanwhile, keeps lines in order and leaves
	// the page's other connections to the local server free.
	let pending = []
	let sending = false
	function flush() {
		const base = window.__AMBERITE_LOCAL__
		if (sending || pending.length === 0 || !base) return
		sending = true
		const body = JSON.stringify(pending)
		pending = []
		fetch(`${base}/app/webview-log`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body,
		})
			.catch(() => {})
			.finally(() => {
				sending = false
				flush()
			})
	}

	for (const method of ['log', 'info', 'warn', 'error', 'debug', 'trace']) {
		const original = console[method]
		if (typeof original !== 'function') continue

		console[method] = function (...args) {
			const result = original.apply(this, args)
			try {
				// The local server's init script sets the base; it may run after this one.
				if (!window.__AMBERITE_LOCAL__) return result
				if (method === 'trace') args.push(new Error('console.trace').stack)
				pending.push({
					level: method === 'log' ? 'info' : method,
					message: args.map(serialize).join(' '),
				})
				queueMicrotask(flush)
			} catch {}
			return result
		}
	}
})()
