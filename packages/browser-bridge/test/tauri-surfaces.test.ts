import { getVersion } from '@tauri-apps/api/app'
import { Channel, convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { join } from '@tauri-apps/api/path'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { open, save } from '@tauri-apps/plugin-dialog'
import {
	mkdir,
	readDir,
	readFile,
	readTextFile,
	remove,
	rename,
	stat,
	writeFile,
	writeTextFile,
} from '@tauri-apps/plugin-fs'
import { fetch as tauriFetch } from '@tauri-apps/plugin-http'
import { openUrl } from '@tauri-apps/plugin-opener'
import { platform, type as osType, version } from '@tauri-apps/plugin-os'
import { saveWindowState, StateFlags } from '@tauri-apps/plugin-window-state'
import { afterAll, beforeEach, describe, expect, it, vi } from 'vitest'

import {
	type BridgeRequest,
	type BrowserWindow,
	installBrowserBridge,
	MockWebSocket,
} from './harness'

const TAG = '__amberiteBrowserBridge'
const originalWindow = Object.getOwnPropertyDescriptor(globalThis, 'window')
const originalWebSocket = Object.getOwnPropertyDescriptor(globalThis, 'WebSocket')
let socket: MockWebSocket
let bridgeWindow: BrowserWindow

beforeEach(async () => {
	const bridge = await installBrowserBridge()
	socket = bridge.socket
	bridgeWindow = bridge.window
	Object.defineProperty(globalThis, 'window', { configurable: true, value: bridge.window })
	Object.defineProperty(globalThis, 'WebSocket', { configurable: true, value: MockWebSocket })
})

afterAll(() => {
	restoreGlobal('window', originalWindow)
	restoreGlobal('WebSocket', originalWebSocket)
})

describe.sequential('Tauri surfaces used by Amberite', () => {
	it('covers app, core, path, opener, dialog, OS, and window-state APIs', async () => {
		await roundTrip(getVersion(), 'plugin:app|version', {}, '1.0.0-local')
		await roundTrip(
			join('C:\\Amberite', 'profiles'),
			'plugin:path|join',
			{
				paths: ['C:\\Amberite', 'profiles'],
			},
			'C:\\Amberite\\profiles',
		)
		await roundTrip(
			open({ directory: true, multiple: false }),
			'plugin:dialog|open',
			{
				options: { directory: true, multiple: false },
			},
			'C:\\Amberite',
		)
		await roundTrip(
			save({ defaultPath: 'export.mrpack' }),
			'plugin:dialog|save',
			{
				options: { defaultPath: 'export.mrpack' },
			},
			'C:\\export.mrpack',
		)
		await roundTrip(
			openUrl('https://modrinth.com'),
			'plugin:opener|open_url',
			{
				url: 'https://modrinth.com',
				with: undefined,
			},
			undefined,
		)
		await roundTrip(
			saveWindowState(StateFlags.ALL),
			'plugin:window-state|save_window_state',
			{
				flags: 63,
			},
			undefined,
		)

		expect(platform()).toBe('windows')
		expect(osType()).toBe('windows')
		expect(version()).toBe('11.0.0')
		expect(convertFileSrc('C:\\icon.png')).toContain('__amberite_browser_bridge_asset')

		const direct = invoke('custom_command', { exact: 'args' })
		await roundTrip(direct, 'custom_command', { exact: 'args' }, { exact: 'result' })
	})

	it('covers every window command and event Amberite uses', async () => {
		const current = getCurrentWindow()
		expect(current.label).toBe('main')
		await roundTrip(current.minimize(), 'plugin:window|minimize', { label: 'main' }, undefined)
		await roundTrip(
			current.toggleMaximize(),
			'plugin:window|toggle_maximize',
			{ label: 'main' },
			undefined,
		)
		await roundTrip(
			current.setDecorations(false),
			'plugin:window|set_decorations',
			{ label: 'main', value: false },
			undefined,
		)
		await roundTrip(current.isMaximized(), 'plugin:window|is_maximized', { label: 'main' }, true)
		await roundTrip(current.close(), 'plugin:window|close', { label: 'main' }, undefined)

		const resized = vi.fn()
		const listening = current.onResized(resized)
		const request = await socket.nextInvoke()
		expect(request.cmd).toBe('plugin:event|listen')
		expect(request.args).toMatchObject({
			event: 'tauri://resize',
			target: { kind: 'Window', label: 'main' },
		})
		const callbackId = callbackIdFrom(request)
		socket.respond(request, 'success', 71)
		const unlisten = await listening
		socket.callback(callbackId, {
			event: 'tauri://resize',
			id: 71,
			payload: { height: 720, width: 1280 },
		})
		expect(resized).toHaveBeenCalledOnce()
		expect(resized.mock.calls[0]?.[0].payload).toMatchObject({ height: 720, width: 1280 })
		const unlistening = unlisten()
		const unlistenRequest = await socket.nextInvoke()
		expect(unlistenRequest).toMatchObject({
			args: { event: 'tauri://resize', eventId: 71 },
			cmd: 'plugin:event|unlisten',
		})
		socket.respond(unlistenRequest, 'success', null)
		await unlistening
	})

	it('covers generic events, Webview drag/drop, and channels', async () => {
		const eventHandler = vi.fn()
		const listening = listen('amberite://test', eventHandler)
		const listenRequest = await socket.nextInvoke()
		const eventCallbackId = callbackIdFrom(listenRequest)
		socket.respond(listenRequest, 'success', 8)
		await listening
		socket.callback(eventCallbackId, { event: 'amberite://test', id: 8, payload: { ready: true } })
		expect(eventHandler).toHaveBeenCalledWith({
			event: 'amberite://test',
			id: 8,
			payload: { ready: true },
		})

		const channelHandler = vi.fn()
		const channel = new Channel(channelHandler)
		const channelInvocation = invoke('plugin:test|channel', { channel })
		const channelRequest = await socket.nextInvoke()
		const channelWire = (channelRequest.args as { channel: { id: number } }).channel
		expect(channelWire).toEqual({ [TAG]: 'channel', id: channel.id })
		socket.respond(channelRequest, 'success', null)
		await channelInvocation
		socket.callback(channel.id, { index: 1, message: 'second' })
		socket.callback(channel.id, { index: 0, message: 'first' })
		expect(channelHandler.mock.calls.map((call) => call[0])).toEqual(['first', 'second'])

		const dropped = vi.fn()
		const dragListening = getCurrentWebview().onDragDropEvent(dropped)
		const dragCallbacks = new Map<string, number>()
		for (const event of [
			'tauri://drag-enter',
			'tauri://drag-over',
			'tauri://drag-drop',
			'tauri://drag-leave',
		]) {
			const request = await socket.nextInvoke()
			expect(request).toMatchObject({ cmd: 'plugin:event|listen' })
			expect((request.args as { event: string }).event).toBe(event)
			dragCallbacks.set(event, callbackIdFrom(request))
			socket.respond(request, 'success', dragCallbacks.size + 100)
		}
		await dragListening
		socket.callback(dragCallbacks.get('tauri://drag-drop')!, {
			event: 'tauri://drag-drop',
			id: 103,
			payload: { paths: ['C:\\mod.jar'], position: { x: 20, y: 30 } },
		})
		expect(dropped).toHaveBeenCalledWith(
			expect.objectContaining({
				payload: expect.objectContaining({ paths: ['C:\\mod.jar'], type: 'drop' }),
			}),
		)
	})

	it('covers all filesystem functions, including raw writes and binary reads', async () => {
		await roundTrip(
			mkdir('mods', { recursive: true }),
			'plugin:fs|mkdir',
			{
				options: { recursive: true },
				path: 'mods',
			},
			undefined,
		)
		await roundTrip(readDir('mods'), 'plugin:fs|read_dir', { options: undefined, path: 'mods' }, [
			{ isDirectory: false, isFile: true, isSymlink: false, name: 'a.jar' },
		])

		const binaryRead = readFile('icon.png')
		const binaryRequest = await socket.nextInvoke()
		expect(binaryRequest).toMatchObject({ cmd: 'plugin:fs|read_file' })
		socket.respond(binaryRequest, 'success', { [TAG]: 'arrayBuffer', data: 'AAH/' })
		await expect(binaryRead).resolves.toEqual(new Uint8Array([0, 1, 255]))

		const textRead = readTextFile('config.json')
		const textRequest = await socket.nextInvoke()
		socket.respond(textRequest, 'success', { [TAG]: 'arrayBuffer', data: 'eyJvayI6dHJ1ZX0=' })
		await expect(textRead).resolves.toBe('{"ok":true}')

		await roundTrip(
			remove('old.jar'),
			'plugin:fs|remove',
			{
				options: undefined,
				path: 'old.jar',
			},
			undefined,
		)
		await roundTrip(
			rename('old', 'new'),
			'plugin:fs|rename',
			{
				newPath: 'new',
				oldPath: 'old',
				options: undefined,
			},
			undefined,
		)
		await roundTrip(
			stat('new'),
			'plugin:fs|stat',
			{ options: undefined, path: 'new' },
			{
				atime: null,
				birthtime: null,
				isDirectory: false,
				isFile: true,
				isSymlink: false,
				mtime: null,
				readonly: false,
				size: 3,
			},
		)

		const rawWrite = writeFile('bytes.bin', new Uint8Array([1, 2, 255]), { create: true })
		const rawRequest = await socket.nextInvoke()
		expect(rawRequest.cmd).toBe('plugin:fs|write_file')
		expect(rawRequest.args).toEqual({ [TAG]: 'bytes', data: 'AQL/' })
		expect(rawRequest.options).toEqual({
			headers: { options: '{"create":true}', path: 'bytes.bin' },
		})
		socket.respond(rawRequest, 'success', null)
		await rawWrite

		const textWrite = writeTextFile('text.txt', 'Amberite')
		const textWriteRequest = await socket.nextInvoke()
		expect(textWriteRequest.cmd).toBe('plugin:fs|write_text_file')
		expect(textWriteRequest.args).toEqual({ [TAG]: 'bytes', data: 'QW1iZXJpdGU=' })
		expect(textWriteRequest.options).toEqual({
			headers: { options: { [TAG]: 'undefined' }, path: 'text.txt' },
		})
		socket.respond(textWriteRequest, 'success', null)
		await textWrite
	})

	it('covers the Tauri HTTP adapter command sequence and arguments', async () => {
		const fetching = tauriFetch('https://example.com/api', {
			body: JSON.stringify({ hello: 'world' }),
			headers: { 'content-type': 'application/json' },
			method: 'POST',
		})
		const createRequest = await socket.nextInvoke()
		expect(createRequest).toMatchObject({ cmd: 'plugin:http|fetch' })
		expect(createRequest.args).toMatchObject({
			clientConfig: {
				data: Array.from(new TextEncoder().encode('{"hello":"world"}')),
				method: 'POST',
				url: 'https://example.com/api',
			},
		})
		socket.respond(createRequest, 'success', 501)
		const sendRequest = await socket.nextInvoke()
		expect(sendRequest).toMatchObject({ args: { rid: 501 }, cmd: 'plugin:http|fetch_send' })
		socket.respond(sendRequest, 'success', {
			headers: [['content-type', 'application/json']],
			rid: 502,
			status: 200,
			statusText: 'OK',
			url: 'https://example.com/api',
		})
		const response = await fetching
		expect(response.status).toBe(200)
		expect(response.url).toBe('https://example.com/api')
	})

	it('rejects through every imported wrapper, not only direct invoke', async () => {
		const rejected = getVersion().catch((error: unknown) => error)
		const request = await socket.nextInvoke()
		socket.respond(request, 'error', { code: 'native-error', message: 'version unavailable' })
		await expect(rejected).resolves.toEqual({
			code: 'native-error',
			message: 'version unavailable',
		})
	})
})

async function roundTrip<T>(
	promise: Promise<T>,
	cmd: string,
	args: unknown,
	result: unknown,
): Promise<void> {
	const request = await socket.nextInvoke()
	expect(request).toMatchObject({ cmd })
	expect(request.args).toEqual(bridgeWindow.__AMBERITE_BROWSER_BRIDGE_TEST__.toWire(args))
	socket.respond(request, 'success', bridgeWindow.__AMBERITE_BROWSER_BRIDGE_TEST__.toWire(result))
	await expect(promise).resolves.toEqual(result)
}

function callbackIdFrom(request: BridgeRequest): number {
	const handler = (request.args as { handler: { id?: unknown } }).handler
	if (typeof handler.id !== 'number') throw new Error('Expected a bridged callback ID')
	return handler.id
}

function restoreGlobal(
	name: 'WebSocket' | 'window',
	descriptor: PropertyDescriptor | undefined,
): void {
	if (descriptor) Object.defineProperty(globalThis, name, descriptor)
	else Reflect.deleteProperty(globalThis, name)
}
