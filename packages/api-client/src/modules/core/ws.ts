import type { CoreInstanceStatus, CoreStats, CoreWsFrame } from './types'

type EventMap = {
	log: string
	stats: CoreStats
	state: CoreInstanceStatus
	open: void
	close: void
	error: Event
}

type Listener<K extends keyof EventMap> = (data: EventMap[K]) => void

/**
 * Typed WebSocket connection to a single Core instance console stream.
 *
 * Usage:
 *   const ws = new CoreWsConnection(wsUrl)
 *   ws.on('log', line => ...)
 *   ws.on('stats', stats => ...)
 *   ws.on('state', status => ...)
 *   ws.send('say hello')
 *   ws.close()
 */
export class CoreWsConnection {
	private socket: WebSocket
	private listeners: { [K in keyof EventMap]?: Array<Listener<K>> } = {}

	constructor(url: string) {
		this.socket = new WebSocket(url)
		this.socket.onopen = () => this.emit('open', undefined as void)
		this.socket.onclose = () => this.emit('close', undefined as void)
		this.socket.onerror = (e) => this.emit('error', e)
		this.socket.onmessage = (e) => this.handleMessage(e.data)
	}

	on<K extends keyof EventMap>(type: K, cb: Listener<K>): () => void {
		if (!this.listeners[type]) this.listeners[type] = []
		;(this.listeners[type] as Array<Listener<K>>).push(cb)
		return () => this.off(type, cb)
	}

	off<K extends keyof EventMap>(type: K, cb: Listener<K>): void {
		const arr = this.listeners[type] as Array<Listener<K>> | undefined
		if (!arr) return
		const idx = arr.indexOf(cb)
		if (idx !== -1) arr.splice(idx, 1)
	}

	/** Send a console command to the server. */
	send(command: string): void {
		if (this.socket.readyState === WebSocket.OPEN) {
			this.socket.send(command)
		}
	}

	close(): void {
		this.socket.close()
	}

	get readyState(): number {
		return this.socket.readyState
	}

	private emit<K extends keyof EventMap>(type: K, data: EventMap[K]): void {
		const arr = this.listeners[type] as Array<Listener<K>> | undefined
		if (!arr) return
		for (const cb of arr) cb(data)
	}

	private handleMessage(raw: string): void {
		let frame: CoreWsFrame
		try {
			frame = JSON.parse(raw) as CoreWsFrame
		} catch {
			// Fallback: treat unparsable messages as plain log lines (backwards compat).
			this.emit('log', raw)
			return
		}

		switch (frame.type) {
			case 'log':
				this.emit('log', frame.data)
				break
			case 'stats':
				this.emit('stats', frame.data)
				break
			case 'state':
				this.emit('state', frame.data.status)
				break
		}
	}
}
