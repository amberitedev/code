/** An event as a host's `/events` socket sends it: a `type`, and a `payload` unless it has none. */
type Typed = { type: string }

export type PayloadOf<Event extends Typed, Type extends Event['type']> =
	Extract<Event, { type: Type }> extends { payload: infer Payload } ? Payload : undefined

/** How long a closed socket waits before it opens again, doubling up to `MAX_REOPEN_MS`. */
const FIRST_REOPEN_MS = 1000
const MAX_REOPEN_MS = 30_000

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
 * listener or subscription and reopens after it drops. A reopen hands listeners a `resync`, since
 * events sent meanwhile are lost; the first open needs none. Topics are counted, so each is
 * subscribed once however many screens want it, and subscribed again on every open.
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
		return () => handlers.delete(untyped)
	}

	/** Every event, as it arrives. */
	onAny(handler: (event: Event) => void): () => void {
		this.everything.add(handler)
		this.start()
		return () => this.everything.delete(handler)
	}

	/** Told `true` once the socket opens, and `false` each time it drops or fails to open. */
	onStatus(handler: (open: boolean) => void): () => void {
		this.statusListeners.add(handler)
		this.start()
		return () => this.statusListeners.delete(handler)
	}

	/** Resolves once the socket is open, so an event raised from then on is not missed. */
	ready(): Promise<void> {
		if (this.isOpen) return Promise.resolve()
		return new Promise((resolve) => {
			const stop = this.onStatus((open) => {
				if (!open) return
				stop()
				resolve()
			})
		})
	}

	/** Subscribe to `topic` until the returned function is called. */
	subscribe(topic: Topic): () => void {
		const key = JSON.stringify(topic)
		const held = this.topics.get(key)
		if (held) held.count++
		else {
			this.topics.set(key, { topic, count: 1 })
			this.sendTopic(topic, true)
		}
		this.start()
		let done = false
		return () => {
			if (done) return
			done = true
			const entry = this.topics.get(key)
			if (!entry || --entry.count > 0) return
			this.topics.delete(key)
			this.sendTopic(topic, false)
		}
	}

	/** Close the socket for good. */
	close(): void {
		this.closed = true
		clearTimeout(this.reopenTimer)
		this.socket?.close(1000)
		this.socket = undefined
	}

	private start(): void {
		if (this.socket || this.connecting || this.reopenTimer || this.closed) return
		void this.connect()
	}

	private async connect(): Promise<void> {
		this.reopenTimer = undefined
		this.connecting = true
		let url: string
		try {
			url = await this.source.url()
		} catch {
			this.connecting = false
			this.scheduleReopen()
			return
		}
		this.connecting = false
		if (this.closed) return
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
			this.isOpen = false
			for (const handler of [...this.statusListeners]) handler(false)
			this.scheduleReopen()
		}
	}

	private scheduleReopen(): void {
		if (this.closed) return
		const wait = this.reopenAfter
		this.reopenAfter = Math.min(this.reopenAfter * 2, MAX_REOPEN_MS)
		this.reopenTimer = setTimeout(async () => {
			this.connecting = true
			await this.source.beforeReopen?.().catch(() => undefined)
			this.connecting = false
			void this.connect()
		}, wait)
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
