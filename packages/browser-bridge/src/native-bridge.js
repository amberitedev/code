// Runs inside Amberite's native Tauri webview. Kept standalone so the exact dispatcher is testable.
;(function installAmberiteNativeBridgeDispatcher() {
	'use strict'

	if (window.__BROWSER_BRIDGE_NATIVE__) return

	const TAG = '__amberiteBrowserBridge'
	const callbackIds = new Map()

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
		if (ancestors.has(input)) throw new TypeError('Cannot bridge a circular Tauri result')
		ancestors.add(input)
		const output = Array.isArray(input) ? [] : {}
		for (const [key, value] of Object.entries(input)) output[key] = toWire(value, ancestors)
		ancestors.delete(input)
		return output
	}

	// Rejects instead of throwing, so a page without Tauri or an unserializable payload always
	// reaches the caller's catch.
	async function emit(event, payload) {
		await window.__TAURI_INTERNALS__.invoke('plugin:event|emit', { event, payload })
	}

	function reportEmitFailure(error) {
		console.error('Amberite browser bridge could not emit a transport message', error)
	}

	// Every invoke must be answered. If the answer itself cannot be sent, say so and try once more
	// with a plain error the transport can always carry.
	function respond(request, status, payload) {
		const envelope = { connectionId: request.connectionId, id: request.id }
		emit(request.responseEvent, { ...envelope, payload, status })
			.catch((error) => {
				reportEmitFailure(error)
				return emit(request.responseEvent, {
					...envelope,
					payload: toWire(
						new Error(`The App could not send its answer to ${request.cmd}: ${String(error)}`),
					),
					status: 'error',
				})
			})
			.catch(reportEmitFailure)
	}

	function callbackKey(connectionId, callbackId) {
		return `${connectionId}:${callbackId}`
	}

	function getCallback(connectionId, callbackId, callbackEvent) {
		const key = callbackKey(connectionId, callbackId)
		const existing = callbackIds.get(key)
		if (existing !== undefined) return existing
		let nativeId
		nativeId = window.__TAURI_INTERNALS__.transformCallback((payload) => {
			emit(callbackEvent, { callbackId, connectionId, payload: toWire(payload) }).catch(
				reportEmitFailure,
			)
			if (payload && typeof payload === 'object' && 'end' in payload) {
				window.__TAURI_INTERNALS__.unregisterCallback(nativeId)
				callbackIds.delete(key)
			}
		})
		callbackIds.set(key, nativeId)
		return nativeId
	}

	function fromWire(input, request) {
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
		if (input[TAG] === 'callback') {
			return getCallback(request.connectionId, input.id, request.callbackEvent)
		}
		if (input[TAG] === 'channel') {
			return `__CHANNEL__:${getCallback(request.connectionId, input.id, request.callbackEvent)}`
		}
		if (Array.isArray(input)) return input.map((value) => fromWire(value, request))
		const output = {}
		for (const [key, value] of Object.entries(input)) output[key] = fromWire(value, request)
		return output
	}

	async function invoke(request) {
		let status = 'success'
		let payload
		try {
			payload = toWire(
				await window.__TAURI_INTERNALS__.invoke(
					request.cmd,
					fromWire(request.args, request),
					fromWire(request.options, request),
				),
			)
		} catch (error) {
			status = 'error'
			try {
				payload = toWire(error)
			} catch {
				payload = toWire(new Error(String(error)))
			}
		}
		respond(request, status, payload)
	}

	// Entry point for the App's eval. Its return value tells Rust whether this page can answer at
	// all, because an error page left behind by a failed reload has no Tauri to invoke or emit with.
	function dispatch(request) {
		if (!window.__TAURI_INTERNALS__) return 'no-tauri'
		void invoke(request)
		return 'ok'
	}

	function unregister(connectionId, callbackId) {
		const key = callbackKey(connectionId, callbackId)
		const nativeId = callbackIds.get(key)
		if (nativeId === undefined) return
		window.__TAURI_INTERNALS__.unregisterCallback(nativeId)
		callbackIds.delete(key)
	}

	function clear(connectionId) {
		const prefix = `${connectionId}:`
		for (const [key, nativeId] of callbackIds) {
			if (!key.startsWith(prefix)) continue
			window.__TAURI_INTERNALS__.unregisterCallback(nativeId)
			callbackIds.delete(key)
		}
	}

	Object.defineProperty(window, '__BROWSER_BRIDGE_NATIVE__', {
		value: { clear, dispatch, invoke, unregister },
	})
})()
