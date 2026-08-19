import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'
import * as NodeVM from 'node:vm'
import { fileURLToPath } from 'node:url'

export type BridgeRequest = {
	readonly args: unknown
	readonly cmd: string
	readonly id: number
	readonly options: unknown
	readonly type: 'invoke'
}

type CallbackMessage = {
	readonly callbackId: number
	readonly type: 'unregisterCallback'
}

type SentMessage = BridgeRequest | CallbackMessage

type BrowserBridgeTestApi = {
	readonly fromWire: (value: unknown) => unknown
	readonly ready: Promise<void>
	readonly socket: () => MockWebSocket
	readonly toWire: (value: unknown) => unknown
}

export type BrowserWindow = Record<string, unknown> & {
	__AMBERITE_BROWSER_BRIDGE_TEST__: BrowserBridgeTestApi
	__TAURI_INTERNALS__: {
		callbacks: Map<number, (value: unknown) => unknown>
		convertFileSrc: (path: string, protocol?: string) => string
		invoke: <T>(cmd: string, args?: unknown, options?: unknown) => Promise<T>
		metadata: {
			currentWebview: { label: string }
			currentWindow: { label: string }
		}
		plugins: { path: { delimiter: string; sep: string } }
		runCallback: (id: number, value: unknown) => void
		transformCallback: (callback: (value: unknown) => unknown, once?: boolean) => number
		unregisterCallback: (id: number) => void
	}
	__TAURI_OS_PLUGIN_INTERNALS__: Record<string, string>
}

export class MockWebSocket {
	static readonly OPEN = 1
	static latest: MockWebSocket | undefined

	onclose: (() => void) | null = null
	onerror: ((error: unknown) => void) | null = null
	onmessage: ((event: { data: string }) => void) | null = null
	onopen: (() => void) | null = null
	readyState = 0
	readonly sent: SentMessage[] = []
	private readonly waiters: Array<(message: SentMessage) => void> = []

	constructor(readonly url: string) {
		MockWebSocket.latest = this
		queueMicrotask(() => {
			this.readyState = MockWebSocket.OPEN
			this.onopen?.()
		})
	}

	send(raw: string): void {
		const message = JSON.parse(raw) as SentMessage
		const waiter = this.waiters.shift()
		if (waiter) waiter(message)
		else this.sent.push(message)
	}

	async next(): Promise<SentMessage> {
		const message = this.sent.shift()
		if (message) return message
		return new Promise((resolve) => this.waiters.push(resolve))
	}

	async nextInvoke(): Promise<BridgeRequest> {
		for (;;) {
			const message = await this.next()
			if (message.type === 'invoke') return message
		}
	}

	respond(request: BridgeRequest, status: 'error' | 'success', payload: unknown): void {
		this.onmessage?.({
			data: JSON.stringify({
				id: request.id,
				payload,
				status,
				type: 'response',
			}),
		})
	}

	callback(callbackId: number, payload: unknown): void {
		this.onmessage?.({
			data: JSON.stringify({ callbackId, payload, type: 'callback' }),
		})
	}
}

export async function installBrowserBridge(): Promise<{
	socket: MockWebSocket
	window: BrowserWindow
}> {
	const window = {
		__AMBERITE_BROWSER_BRIDGE_CONFIG__: {
			assetPath: '/__amberite_browser_bridge_asset',
			currentWebviewLabel: 'main',
			currentWindowLabel: 'main',
			os: {
				arch: 'x86_64',
				eol: '\r\n',
				exeExtension: 'exe',
				family: 'windows',
				platform: 'windows',
				type: 'windows',
				version: '11.0.0',
			},
			pathDelimiter: ';',
			pathSeparator: '\\',
			wsPath: '/__amberite_browser_bridge_ws',
		},
		crypto: globalThis.crypto,
		location: {
			host: '127.0.0.1:43123',
			origin: 'http://127.0.0.1:43123',
			protocol: 'http:',
		},
	} as unknown as BrowserWindow

	const context = {
		ArrayBuffer,
		Error,
		Headers,
		Map,
		TextDecoder,
		TextEncoder,
		Uint32Array,
		Uint8Array,
		URL,
		WeakSet,
		WebSocket: MockWebSocket,
		atob,
		btoa,
		console,
		queueMicrotask,
		window,
	}
	NodeVM.runInNewContext(readSource('browser-bridge.js'), context, {
		filename: 'browser-bridge.js',
	})
	await window.__AMBERITE_BROWSER_BRIDGE_TEST__.ready
	const socket = MockWebSocket.latest
	if (!socket) throw new Error('Browser bridge did not create a WebSocket')
	return { socket, window }
}

export type NativeInvocation = {
	readonly args: unknown
	readonly cmd: string
	readonly options: unknown
}

type NativeDispatcher = {
	clear: (connectionId: number) => void
	invoke: (request: Record<string, unknown>) => Promise<void>
	unregister: (connectionId: number, callbackId: number) => void
}

export function installNativeBridge(handler: (invocation: NativeInvocation) => unknown) {
	const callbacks = new Map<number, (value: unknown) => void>()
	const emitted: Array<{ event: string; payload: unknown }> = []
	const invocations: NativeInvocation[] = []
	let nextCallbackId = 10_000
	const nativeInternals = {
		invoke: async (cmd: string, args: unknown, options: unknown) => {
			if (cmd === 'plugin:event|emit') {
				const envelope = args as { event: string; payload: unknown }
				emitted.push(envelope)
				return undefined
			}
			const invocation = { args, cmd, options }
			invocations.push(invocation)
			return handler(invocation)
		},
		transformCallback: (callback: (value: unknown) => void) => {
			const id = ++nextCallbackId
			callbacks.set(id, callback)
			return id
		},
		unregisterCallback: (id: number) => callbacks.delete(id),
	}
	const window = { __TAURI_INTERNALS__: nativeInternals } as Record<string, unknown>
	NodeVM.runInNewContext(
		readSource('native-bridge.js'),
		{
			ArrayBuffer,
			Error,
			Map,
			Uint8Array,
			WeakSet,
			atob,
			btoa,
			console,
			window,
		},
		{ filename: 'native-bridge.js' },
	)
	const dispatcher = window.__AMBERITE_BROWSER_BRIDGE_NATIVE__ as NativeDispatcher
	return { callbacks, dispatcher, emitted, invocations }
}

export async function flushTasks(): Promise<void> {
	await new Promise((resolve) => setTimeout(resolve, 0))
}

function readSource(name: string): string {
	const directory = NodePath.dirname(fileURLToPath(import.meta.url))
	return NodeFS.readFileSync(NodePath.join(directory, '..', 'src', name), 'utf8')
}
