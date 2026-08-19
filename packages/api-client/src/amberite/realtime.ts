export type RealtimeFrame =
	| { type: 'presence.snapshot'; users: Record<string, boolean> }
	| { type: 'presence.user'; userId: string; online: boolean }
	| { type: 'authorization.invalidated' }

export interface RealtimeSocket {
	readonly CONNECTING: number
	readonly OPEN: number
	readonly readyState: number
	onopen: ((event: Event) => void) | null
	onmessage: ((event: MessageEvent<unknown>) => void) | null
	onclose: ((event: CloseEvent) => void) | null
	onerror: ((event: Event) => void) | null
	close(code?: number, reason?: string): void
}

export interface RealtimePresenceSessionOptions {
	endpoint: string
	fetchFn: typeof fetch
	createWebSocket: (url: string) => RealtimeSocket
	getJwt: () => Promise<string | null>
	origin?: string
	maxReconnectAttempts?: number
	onFrame: (frame: RealtimeFrame) => void
	onInvalidated: () => void
}

/** Authenticated, ephemeral user presence. It never models Core availability. */
export class RealtimePresenceSession {
	private socket: RealtimeSocket | null = null
	private reconnectTimer: ReturnType<typeof setTimeout> | null = null
	private expiryTimer: ReturnType<typeof setTimeout> | null = null
	private attempts = 0
	private disposed = false

	constructor(private readonly options: RealtimePresenceSessionOptions) {}

	async connect(): Promise<void> {
		if (this.disposed || this.isConnectingOrOpen()) return
		try {
			const token = await this.options.getJwt()
			if (!token || this.disposed) return
			const endpoint = this.endpointUrl()
			const response = await this.options.fetchFn(new URL('v1/desktop-sessions', endpoint), {
				method: 'POST',
				headers: {
					Authorization: `Bearer ${token}`,
					...(this.options.origin ? { Origin: this.options.origin } : {}),
				},
			})
			if (!response.ok) throw new Error(`realtime ticket request failed (${response.status})`)
			const ticket = parseTicket(await response.json())
			if (!ticket) throw new Error('realtime ticket response is invalid')
			const url = new URL('v1/connect', endpoint)
			url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'
			url.searchParams.set('ticket', ticket)
			this.open(this.options.createWebSocket(url.toString()))
			this.scheduleExpiry(jwtExpiry(token))
		} catch {
			this.scheduleReconnect()
		}
	}

	dispose(): void {
		this.disposed = true
		this.clearTimers()
		this.closeSocket(1000, 'Session disposed')
	}

	setEndpoint(endpoint: string): void {
		if (this.options.endpoint === endpoint) return
		this.options.endpoint = endpoint
		this.closeSocket(1000, 'Endpoint changed')
		this.attempts = 0
		void this.connect()
	}

	private endpointUrl() {
		const endpoint = new URL(
			this.options.endpoint.endsWith('/') ? this.options.endpoint : `${this.options.endpoint}/`,
		)
		if (endpoint.protocol !== 'https:' && endpoint.protocol !== 'http:')
			throw new Error('Realtime endpoint must use HTTPS or HTTP')
		return endpoint
	}

	private open(socket: RealtimeSocket) {
		this.closeSocket(1000, 'Socket replaced')
		this.socket = socket
		socket.onopen = () => {
			this.attempts = 0
		}
		socket.onmessage = (event) => {
			const frame = parseFrame(event.data)
			if (!frame) return
			if (frame.type === 'authorization.invalidated') {
				this.options.onInvalidated()
				this.dispose()
				return
			}
			this.options.onFrame(frame)
		}
		socket.onclose = () => {
			if (this.socket === socket) this.socket = null
			if (!this.disposed) this.scheduleReconnect()
		}
		socket.onerror = () => socket.close(1011, 'Realtime socket error')
	}

	private isConnectingOrOpen() {
		return Boolean(
			this.socket &&
			(this.socket.readyState === this.socket.CONNECTING ||
				this.socket.readyState === this.socket.OPEN),
		)
	}

	private scheduleReconnect() {
		if (
			this.disposed ||
			this.reconnectTimer ||
			this.attempts >= (this.options.maxReconnectAttempts ?? 8)
		)
			return
		const delay = Math.min(30_000, 500 * 2 ** this.attempts++)
		this.reconnectTimer = setTimeout(() => {
			this.reconnectTimer = null
			void this.connect()
		}, delay)
	}

	private scheduleExpiry(expiresAt: number | null) {
		if (!expiresAt) return
		if (this.expiryTimer) clearTimeout(this.expiryTimer)
		this.expiryTimer = setTimeout(
			() => this.closeSocket(4001, 'JWT expired'),
			Math.max(0, expiresAt - Date.now()),
		)
	}

	private closeSocket(code: number, reason: string) {
		const socket = this.socket
		this.socket = null
		socket?.close(code, reason)
	}

	private clearTimers() {
		if (this.reconnectTimer) clearTimeout(this.reconnectTimer)
		if (this.expiryTimer) clearTimeout(this.expiryTimer)
		this.reconnectTimer = null
		this.expiryTimer = null
	}
}

function parseTicket(value: unknown) {
	return isRecord(value) && typeof value.ticket === 'string' && /^[a-f0-9]{32}$/.test(value.ticket)
		? value.ticket
		: null
}

function parseFrame(value: unknown): RealtimeFrame | null {
	if (typeof value !== 'string') return null
	try {
		const frame: unknown = JSON.parse(value)
		if (!isRecord(frame) || typeof frame.type !== 'string') return null
		if (frame.type === 'authorization.invalidated') return { type: frame.type }
		if (
			frame.type === 'presence.user' &&
			typeof frame.userId === 'string' &&
			typeof frame.online === 'boolean'
		)
			return { type: frame.type, userId: frame.userId, online: frame.online }
		if (frame.type === 'presence.snapshot' && validUsers(frame.users))
			return { type: frame.type, users: frame.users }
	} catch {
		return null
	}
	return null
}

function jwtExpiry(token: string) {
	const payload = token.split('.')[1]
	if (!payload) return null
	try {
		const parsed: unknown = JSON.parse(atob(payload.replace(/-/g, '+').replace(/_/g, '/')))
		return isRecord(parsed) && typeof parsed.exp === 'number' ? parsed.exp * 1000 : null
	} catch {
		return null
	}
}

function validUsers(value: unknown): value is Record<string, boolean> {
	return isRecord(value) && Object.values(value).every((online) => typeof online === 'boolean')
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null
}
