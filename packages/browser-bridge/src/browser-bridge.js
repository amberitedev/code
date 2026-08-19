// Amberite's development-only browser bridge.
;(function installAmberiteBrowserBridge() {
	'use strict'

	if (window.__TAURI_INTERNALS__?.invoke) return

	const config = window.__AMBERITE_BROWSER_BRIDGE_CONFIG__
	if (!config) throw new Error('Amberite browser bridge configuration is missing')

	const TAG = '__amberiteBrowserBridge'
	const SERIALIZE_TO_IPC_FN = '__TAURI_TO_IPC_KEY__'
	const callbacks = new Map()
	const pending = new Map()
	let nextRequestId = 0
	let socket

	function bytesToBase64(bytes) {
		let binary = ''
		const chunkSize = 0x8000
		for (let offset = 0; offset < bytes.length; offset += chunkSize) {
			binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize))
		}
		return btoa(binary)
	}

	function base64ToBytes(value) {
		const binary = atob(value)
		const bytes = new Uint8Array(binary.length)
		for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index)
		return bytes
	}

	function toWire(input, ancestors = new WeakSet()) {
		if (input === undefined) return { [TAG]: 'undefined' }
		if (typeof input === 'number' && callbacks.has(input)) {
			return { [TAG]: 'callback', id: input }
		}
		if (typeof input === 'string') {
			const channel = /^__CHANNEL__:(\d+)$/.exec(input)
			if (channel && callbacks.has(Number(channel[1]))) {
				return { [TAG]: 'channel', id: Number(channel[1]) }
			}
			return input
		}
		if (input === null || typeof input !== 'object') return input
		if (input instanceof Error) {
			return {
				[TAG]: 'error',
				message: input.message,
				name: input.name,
				stack: input.stack,
			}
		}
		if (input instanceof ArrayBuffer) {
			return { [TAG]: 'arrayBuffer', data: bytesToBase64(new Uint8Array(input)) }
		}
		if (ArrayBuffer.isView(input)) {
			return {
				[TAG]: 'bytes',
				data: bytesToBase64(new Uint8Array(input.buffer, input.byteOffset, input.byteLength)),
			}
		}
		if (input instanceof Map) return toWire(Object.fromEntries(input), ancestors)
		if (input instanceof Headers) return toWire(Object.fromEntries(input), ancestors)
		if (SERIALIZE_TO_IPC_FN in input && typeof input[SERIALIZE_TO_IPC_FN] === 'function') {
			return toWire(input[SERIALIZE_TO_IPC_FN](), ancestors)
		}
		if (typeof input.toJSON === 'function') return toWire(input.toJSON(), ancestors)
		if (ancestors.has(input)) throw new TypeError('Cannot invoke Tauri with a circular value')
		ancestors.add(input)
		const output = Array.isArray(input) ? [] : {}
		for (const [key, value] of Object.entries(input)) output[key] = toWire(value, ancestors)
		ancestors.delete(input)
		return output
	}

	function fromWire(input) {
		if (input === null || typeof input !== 'object') return input
		if (input[TAG] === 'undefined') return undefined
		if (input[TAG] === 'arrayBuffer') return base64ToBytes(input.data).buffer
		if (input[TAG] === 'bytes') return base64ToBytes(input.data)
		if (input[TAG] === 'error') {
			const error = new Error(input.message)
			error.name = input.name || 'Error'
			if (input.stack) error.stack = input.stack
			return error
		}
		if (Array.isArray(input)) return input.map(fromWire)
		const output = {}
		for (const [key, value] of Object.entries(input)) output[key] = fromWire(value)
		return output
	}

	function rejectPending(reason) {
		for (const { reject } of pending.values()) reject(reason)
		pending.clear()
	}

	const ready = new Promise((resolve, reject) => {
		const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
		socket = new WebSocket(`${protocol}//${window.location.host}${config.wsPath}`)
		socket.onopen = resolve
		socket.onerror = reject
		socket.onclose = () => rejectPending(new Error('Amberite browser bridge disconnected'))
		socket.onmessage = ({ data }) => {
			if (typeof data !== 'string') return
			let message
			try {
				message = JSON.parse(data)
			} catch (error) {
				console.error('Amberite browser bridge received invalid JSON', error)
				return
			}
			if (message.type === 'response') {
				const request = pending.get(message.id)
				if (!request) return
				pending.delete(message.id)
				const payload = fromWire(message.payload)
				if (message.status === 'success') request.resolve(payload)
				else request.reject(payload)
				return
			}
			if (message.type === 'callback') runCallback(message.callbackId, fromWire(message.payload))
		}
	})

	async function invoke(cmd, args = {}, options) {
		await ready
		if (!socket || socket.readyState !== WebSocket.OPEN) {
			throw new Error('Amberite browser bridge is not connected')
		}
		const id = ++nextRequestId
		return new Promise((resolve, reject) => {
			pending.set(id, { reject, resolve })
			socket.send(
				JSON.stringify({
					type: 'invoke',
					id,
					cmd,
					args: toWire(args),
					options: toWire(options),
				}),
			)
		})
	}

	function transformCallback(callback, once = false) {
		let id
		do id = window.crypto.getRandomValues(new Uint32Array(1))[0]
		while (id === 0 || callbacks.has(id))
		callbacks.set(id, (data) => {
			if (once) unregisterCallback(id)
			return callback?.(data)
		})
		return id
	}

	function unregisterCallback(id) {
		callbacks.delete(id)
		if (socket?.readyState === WebSocket.OPEN) {
			socket.send(JSON.stringify({ type: 'unregisterCallback', callbackId: id }))
		}
	}

	function runCallback(id, data) {
		const callback = callbacks.get(id)
		if (callback) callback(data)
		else console.warn(`[TAURI] Couldn't find callback id ${id}.`)
	}

	function convertFileSrc(filePath, protocol = 'asset') {
		const url = new URL(config.assetPath, window.location.origin)
		url.searchParams.set('path', filePath)
		url.searchParams.set('protocol', protocol)
		return url.toString()
	}

	Object.defineProperty(window, 'isTauri', { value: true })
	Object.defineProperty(window, '__TAURI_INTERNALS__', {
		value: {
			callbacks,
			convertFileSrc,
			invoke,
			metadata: {
				currentWebview: { label: config.currentWebviewLabel },
				currentWindow: { label: config.currentWindowLabel },
			},
			plugins: {
				path: { delimiter: config.pathDelimiter, sep: config.pathSeparator },
			},
			runCallback,
			transformCallback,
			unregisterCallback,
		},
	})
	Object.defineProperty(window, '__TAURI_EVENT_PLUGIN_INTERNALS__', {
		value: { unregisterListener() {} },
	})
	Object.defineProperty(window, '__TAURI_OS_PLUGIN_INTERNALS__', {
		value: {
			arch: config.os.arch,
			eol: config.os.eol,
			exe_extension: config.os.exeExtension,
			family: config.os.family,
			os_type: config.os.type,
			platform: config.os.platform,
			version: config.os.version,
		},
	})
	Object.defineProperty(window, '__AMBERITE_BROWSER_BRIDGE_TEST__', {
		value: { fromWire, ready, socket: () => socket, toWire },
	})
})()
