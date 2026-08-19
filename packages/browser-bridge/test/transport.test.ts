import { describe, expect, it, vi } from 'vitest'

import { flushTasks, installBrowserBridge, installNativeBridge } from './harness'

const TAG = '__amberiteBrowserBridge'

describe.sequential('browser transport', () => {
	it('installs the Tauri globals synchronously with native metadata', async () => {
		const { socket, window } = await installBrowserBridge()
		expect(socket.url).toBe('ws://127.0.0.1:43123/__amberite_browser_bridge_ws')
		expect(window.__TAURI_INTERNALS__.metadata).toEqual({
			currentWebview: { label: 'main' },
			currentWindow: { label: 'main' },
		})
		expect(window.__TAURI_INTERNALS__.plugins.path).toEqual({ delimiter: ';', sep: '\\' })
		expect(window.__TAURI_OS_PLUGIN_INTERNALS__).toMatchObject({
			os_type: 'windows',
			platform: 'windows',
			version: '11.0.0',
		})
	})

	it('passes commands, nested arguments, invoke options, results, and errors', async () => {
		const { socket, window } = await installBrowserBridge()
		const args = { nested: { enabled: true }, values: ['one', 2, null] }
		const options = { headers: { path: 'C%3A%5Cfile.txt' } }
		const resultPromise = window.__TAURI_INTERNALS__.invoke('plugin:test|roundtrip', args, options)
		const request = await socket.nextInvoke()
		expect(request).toMatchObject({ args, cmd: 'plugin:test|roundtrip', options })
		socket.respond(request, 'success', { answer: 42 })
		await expect(resultPromise).resolves.toEqual({ answer: 42 })

		const errorPromise = window.__TAURI_INTERNALS__.invoke('plugin:test|error', { input: 1 })
		const errorRequest = await socket.nextInvoke()
		socket.respond(errorRequest, 'error', { code: 'denied', message: 'No access' })
		await expect(errorPromise).rejects.toEqual({ code: 'denied', message: 'No access' })
	})

	it('preserves top-level and nested binary data in both directions', async () => {
		const { socket, window } = await installBrowserBridge()
		const bytes = new Uint8Array([0, 1, 127, 128, 255])
		const writePromise = window.__TAURI_INTERNALS__.invoke<ArrayBuffer>(
			'plugin:fs|write_file',
			bytes,
			{ headers: { path: 'binary.dat' } },
		)
		const request = await socket.nextInvoke()
		expect(request.args).toEqual({ [TAG]: 'bytes', data: 'AAF/gP8=' })
		expect(request.options).toEqual({ headers: { path: 'binary.dat' } })
		socket.respond(request, 'success', { [TAG]: 'arrayBuffer', data: '/4ABAA==' })
		const result = await writePromise
		expect(Array.from(new Uint8Array(result))).toEqual([255, 128, 1, 0])

		const nestedPromise = window.__TAURI_INTERNALS__.invoke<{ data: Uint8Array }>('binary-nested')
		const nestedRequest = await socket.nextInvoke()
		socket.respond(nestedRequest, 'success', {
			data: { [TAG]: 'bytes', data: 'CQgH' },
		})
		await expect(nestedPromise).resolves.toMatchObject({ data: new Uint8Array([9, 8, 7]) })
	})

	it('routes callbacks and unregisters them remotely', async () => {
		const { socket, window } = await installBrowserBridge()
		const handler = vi.fn()
		const callbackId = window.__TAURI_INTERNALS__.transformCallback(handler)
		const invokePromise = window.__TAURI_INTERNALS__.invoke('plugin:event|listen', {
			handler: callbackId,
		})
		const request = await socket.nextInvoke()
		expect(request.args).toEqual({ handler: { [TAG]: 'callback', id: callbackId } })
		socket.respond(request, 'success', 7)
		await invokePromise

		socket.callback(callbackId, { event: 'ready', payload: { online: true } })
		expect(handler).toHaveBeenCalledWith({ event: 'ready', payload: { online: true } })
		window.__TAURI_INTERNALS__.unregisterCallback(callbackId)
		await expect(socket.next()).resolves.toEqual({ callbackId, type: 'unregisterCallback' })
		expect(window.__TAURI_INTERNALS__.callbacks.has(callbackId)).toBe(false)
	})

	it('maps convertFileSrc to the scoped loopback asset endpoint', async () => {
		const { window } = await installBrowserBridge()
		const url = new URL(window.__TAURI_INTERNALS__.convertFileSrc('C:\\Amberite Data\\icon.png'))
		expect(url.origin).toBe('http://127.0.0.1:43123')
		expect(url.pathname).toBe('/__amberite_browser_bridge_asset')
		expect(url.searchParams.get('path')).toBe('C:\\Amberite Data\\icon.png')
		expect(url.searchParams.get('protocol')).toBe('asset')
	})
})

describe.sequential('native webview dispatcher', () => {
	it('revives arguments and options and serializes raw native results', async () => {
		const handler = vi.fn((_invocation: unknown) => new Uint8Array([4, 5, 6]).buffer)
		const { dispatcher, emitted } = installNativeBridge(handler)
		await dispatcher.invoke({
			args: { bytes: { [TAG]: 'bytes', data: 'AQID' } },
			callbackEvent: 'bridge://callback',
			cmd: 'plugin:test|binary',
			connectionId: 3,
			options: { headers: { path: 'file.dat' } },
			responseEvent: 'bridge://response',
		})
		expect(handler).toHaveBeenCalledOnce()
		const invocation = handler.mock.calls[0]?.[0]
		expect(invocation).toMatchObject({
			args: { bytes: new Uint8Array([1, 2, 3]) },
			cmd: 'plugin:test|binary',
			options: { headers: { path: 'file.dat' } },
		})
		expect(emitted).toContainEqual({
			event: 'bridge://response',
			payload: { payload: { [TAG]: 'arrayBuffer', data: 'BAUG' }, status: 'success' },
		})
	})

	it('forwards native errors without turning them into successful values', async () => {
		const { dispatcher, emitted } = installNativeBridge(() => {
			throw { code: 'invalid', detail: 'bad input' }
		})
		await dispatcher.invoke({
			args: {},
			callbackEvent: 'bridge://callback',
			cmd: 'plugin:test|fail',
			connectionId: 1,
			options: { [TAG]: 'undefined' },
			responseEvent: 'bridge://response',
		})
		expect(emitted).toContainEqual({
			event: 'bridge://response',
			payload: {
				payload: { code: 'invalid', detail: 'bad input' },
				status: 'error',
			},
		})
	})

	it('translates persistent callbacks and channels to native callback IDs', async () => {
		let nativeArgs: { callback: number; channel: string } | undefined
		const { callbacks, dispatcher, emitted } = installNativeBridge(({ args }) => {
			nativeArgs = args as { callback: number; channel: string }
			return 'registered'
		})
		await dispatcher.invoke({
			args: {
				callback: { [TAG]: 'callback', id: 41 },
				channel: { [TAG]: 'channel', id: 42 },
			},
			callbackEvent: 'bridge://callback',
			cmd: 'plugin:test|callbacks',
			connectionId: 9,
			options: { [TAG]: 'undefined' },
			responseEvent: 'bridge://response',
		})
		if (!nativeArgs) throw new Error('Native args were not captured')
		const channelId = Number(nativeArgs.channel.slice('__CHANNEL__:'.length))
		callbacks.get(nativeArgs.callback)?.({ payload: 'event' })
		callbacks.get(channelId)?.({ index: 0, message: new Uint8Array([7, 8]) })
		await flushTasks()
		expect(emitted).toContainEqual({
			event: 'bridge://callback',
			payload: { callbackId: 41, connectionId: 9, payload: { payload: 'event' } },
		})
		expect(emitted).toContainEqual({
			event: 'bridge://callback',
			payload: {
				callbackId: 42,
				connectionId: 9,
				payload: { index: 0, message: { [TAG]: 'bytes', data: 'Bwg=' } },
			},
		})

		dispatcher.unregister(9, 41)
		expect(callbacks.has(nativeArgs.callback)).toBe(false)
		dispatcher.clear(9)
		expect(callbacks.has(channelId)).toBe(false)
	})
})
