import { ApiCallError } from '../contracts/errors'

/** An event as a host's `/events` socket sends it: a `type`, and a `payload` unless it has none. */
type Typed = { type: string }

export type PayloadOf<Event extends Typed, Type extends Event['type']> =
	Extract<Event, { type: Type }> extends { payload: infer Payload } ? Payload : undefined

/** How long a closed socket waits before it opens again, doubling up to `MAX_REOPEN_MS`. */
const FIRST_REOPEN_MS = 1000
const MAX_REOPEN_MS = 5000
/** How long `ready` waits for the socket to open. */
export const READY_MS = 10_000

/** Where a stream's socket goes, and what it sends to subscribe to a topic. */
export interface EventSource<Topic> {
	/** The socket's `ws:` or `wss:` URL. */
	url(): Promise<string>
	/** The message that subscribes to `topic`, or unsubscribes from it. */
	message?(topic: Topic, subscribe: boolean): unknown
	/** Runs before a reopen, such as a call that signs in again once a session ended. */
	beforeReopen?(): Promise<unknown>
}

/**
 * One `/events` socket, the App's or a Core's, shared by every listener. It opens on the first
 * listener or subscription, reopens after it drops, and closes once nothing listens or subscribes.
 * A reopen hands listeners a `resync`, since events sent meanwhile are lost; the first open needs
 * none. Topics are counted, so each is unsubscribed only with its last holder and subscribed again
 * on every open. A host sends a topic's start (a server's power, a console's backlog) when it is
 * subscribed, so a later holder of a held topic subscribes it afresh, and earlier holders see that
 * start again.
 */
export class EventStream<Event extends Typed, Topic = never> {
	private socket: WebSocket | undefined
	private listeners = new Map<string, Set<(payload: unknown) => void>>()
	private everything = new Set<(event: Event) => void>()
	private statusListeners = new Set<(open: boolean) => void>()
	private topics = new Map<string, { topic: Topic; count: number }>()
	/** Between a connect's start and its socket, while the URL is worked out. */
	private connecting = false
	private opened = false
	private isOpen = false
	private closed = false
	private reopenAfter = FIRST_REOPEN_MS
	private reopenTimer: ReturnType<typeof setTimeout> | undefined

	constructor(private readonly source: EventSource<Topic>) {}

	/** Whether the socket is open now. */
	get open(): boolean {
		return this.isOpen
	}

	on<Type extends Event['type']>(
		type: Type,
		handler: (payload: PayloadOf<Event, Type>) => void,
	): () => void {
		let handlers = this.listeners.get(type)
		if (!handlers) this.listeners.set(type, (handlers = new Set()))
		const untyped = handler as (payload: unknown) => void
		handlers.add(untyped)
		this.start()
		return () => {
			handlers.delete(untyped)
			this.stopWhenIdle()
		}
	}

	/** Every event, as it arrives. */
	onAny(handler: (event: Event) => void): () => void {
		this.everything.add(handler)
		this.start()
		return () => {
			this.everything.delete(handler)
			this.stopWhenIdle()
		}
	}

	/** Told `true` once the socket opens, and `false` each time it drops or fails to open. */
	onStatus(handler: (open: boolean) => void): () => void {
		this.statusListeners.add(handler)
		this.start()
		return () => {
			this.statusListeners.delete(handler)
			this.stopWhenIdle()
		}
	}

	/**
	 * Resolves once the socket is open, so an event raised from then on is not missed. Rejects with
	 * `network_error` when it fails to open, or is not open within `timeout` ms. A socket waiting to
	 * reopen tries again at once: someone is waiting on it.
	 */
	ready(timeout = READY_MS): Promise<void> {
		if (this.isOpen) return Promise.resolve()
		return new Promise((resolve, reject) => {
			const settle = (open: boolean) => {
				clearTimeout(timer)
				stop()
				if (open) resolve()
				else reject(new ApiCallError(0, 'network_error', 'Could not reach the event socket'))
			}
			const timer = setTimeout(() => settle(false), timeout)
			const stop = this.onStatus(settle)
			if (this.reopenTimer) {
				clearTimeout(this.reopenTimer)
				void this.reopen()
			}
		})
	}

	/** Subscribe to `topic` until the returned function is called. */
	subscribe(topic: Topic): () => void {
		const key = JSON.stringify(topic)
		const held = this.topics.get(key)
		if (held) {
			held.count++
			// A host ignores a topic subscribed twice: leave it first to be sent its start again.
			this.sendTopic(topic, false)
		} else this.topics.set(key, { topic, count: 1 })
		this.sendTopic(topic, true)
		this.start()
		let done = false
		return () => {
			if (done) return
			done = true
			const entry = this.topics.get(key)
			if (!entry || --entry.count > 0) return
			this.topics.delete(key)
			this.sendTopic(topic, false)
			this.stopWhenIdle()
		}
	}

	/** Close the socket for good. */
	close(): void {
		this.closed = true
		const wasOpen = this.isOpen
		this.stop()
		if (wasOpen) this.setOpen(false)
	}

	private start(): void {
		if (this.socket || this.connecting || this.reopenTimer || this.closed) return
		void this.connect()
	}

	/** Close the socket once nothing listens or subscribes. The next listener opens it afresh. */
	private stopWhenIdle(): void {
		if (this.wanted()) return
		this.stop()
		this.opened = false
		this.reopenAfter = FIRST_REOPEN_MS
	}

	private stop(): void {
		clearTimeout(this.reopenTimer)
		this.reopenTimer = undefined
		this.socket?.close(1000)
		this.socket = undefined
		this.isOpen = false
	}

	private async connect(): Promise<void> {
		this.reopenTimer = undefined
		this.connecting = true
		let url: string
		try {
			url = await this.source.url()
		} catch {
			this.connecting = false
			this.dropped()
			return
		}
		this.connecting = false
		// Closed, or everyone left, while the URL was worked out.
		if (this.closed || !this.wanted()) return
		const socket = new WebSocket(url)
		this.socket = socket
		socket.onopen = () => {
			if (this.socket !== socket) return
			this.reopenAfter = FIRST_REOPEN_MS
			for (const { topic } of this.topics.values()) this.sendTopic(topic, true)
			const reopened = this.opened
			this.opened = true
			this.setOpen(true)
			if (reopened) this.dispatch({ type: 'resync' } as Event)
		}
		socket.onmessage = (message) => {
			if (this.socket !== socket || typeof message.data !== 'string') return
			this.dispatch(JSON.parse(message.data) as Event)
		}
		socket.onclose = () => {
			if (this.socket !== socket) return
			this.socket = undefined
			this.dropped()
		}
	}

	/** The socket dropped or failed to open: say so, and open it again later. */
	private dropped(): void {
		this.setOpen(false)
		if (this.closed || !this.wanted()) return
		const wait = this.reopenAfter
		this.reopenAfter = Math.min(this.reopenAfter * 2, MAX_REOPEN_MS)
		this.reopenTimer = setTimeout(() => void this.reopen(), wait)
	}

	private async reopen(): Promise<void> {
		this.reopenTimer = undefined
		this.connecting = true
		await this.source.beforeReopen?.().catch(() => undefined)
		this.connecting = false
		if (!this.closed && this.wanted()) void this.connect()
	}

	private wanted(): boolean {
		return (
			this.topics.size > 0 ||
			this.everything.size > 0 ||
			this.statusListeners.size > 0 ||
			[...this.listeners.values()].some((handlers) => handlers.size > 0)
		)
	}

	private setOpen(open: boolean): void {
		this.isOpen = open
		for (const handler of [...this.statusListeners]) handler(open)
	}

	private sendTopic(topic: Topic, subscribe: boolean): void {
		const message = this.source.message?.(topic, subscribe)
		if (message === undefined || this.socket?.readyState !== WebSocket.OPEN) return
		this.socket.send(JSON.stringify(message))
	}

	private dispatch(event: Event): void {
		for (const handler of [...this.everything]) handler(event)
		const handlers = this.listeners.get(event.type)
		if (!handlers) return
		const payload = 'payload' in event ? event.payload : undefined
		for (const handler of [...handlers]) handler(payload)
	}
}
