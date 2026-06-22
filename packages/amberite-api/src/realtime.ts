export type RealtimeCoreHealth = 'healthy' | 'degraded' | 'offline'
export type RealtimeCoreDiagnostic = 'none' | 'network' | 'authentication' | 'server'

export type RealtimeFrame =
	| {
			type: 'presence.snapshot'
			users: Record<string, boolean>
			cores: Record<
				string,
				{ online: boolean; health?: RealtimeCoreHealth; diagnostic?: RealtimeCoreDiagnostic }
			>
	  }
	| { type: 'presence.user'; userId: string; online: boolean }
	| {
			type: 'presence.core'
			coreId: string
			online: boolean
			health?: RealtimeCoreHealth
			diagnostic?: RealtimeCoreDiagnostic
	  }
	| { type: 'authorization.invalidated' }

export interface RealtimeSocket {
	readonly CONNECTING: number
	readonly OPEN: number
	readonly readyState: number
	onopen: (() => void) | null
	onmessage: ((event: { data: unknown }) => void) | null
	onclose: (() => void) | null
	onerror: (() => void) | null
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

/**
 * Owns the authenticated Worker socket for one desktop session. Durable social
 * data and UI state stay with the consumer; this class only acquires tickets,
 * validates frames, reconnects within a bounded budget, and tears down sockets.
 */
export class RealtimePresenceSession {
	private socket: RealtimeSocket | null = null
	private reconnectTimer: ReturnType<typeof setTimeout> | null = null
	private expiryTimer: ReturnType<typeof setTimeout> | null = null
	private attempts = 0
	private disposed = false

	constructor(private options: RealtimePresenceSessionOptions) {}

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

	private endpointUrl(): URL {
		const endpoint = new URL(
			this.options.endpoint.endsWith('/') ? this.options.endpoint : `${this.options.endpoint}/`,
		)
		if (endpoint.protocol !== 'https:' && endpoint.protocol !== 'http:')
			throw new Error('Realtime endpoint must use HTTPS or HTTP')
		return endpoint
	}

	private open(socket: RealtimeSocket): void {
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

	private isConnectingOrOpen(): boolean {
		return (
			this.socket !== null &&
			(this.socket.readyState === this.socket.CONNECTING ||
				this.socket.readyState === this.socket.OPEN)
		)
	}

	private scheduleReconnect(): void {
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

	private scheduleExpiry(expiresAt: number | null): void {
		if (!expiresAt) return
		if (this.expiryTimer) clearTimeout(this.expiryTimer)
		const delay = Math.max(0, expiresAt - Date.now())
		this.expiryTimer = setTimeout(() => this.closeSocket(4001, 'JWT expired'), delay)
	}

	private closeSocket(code: number, reason: string): void {
		const socket = this.socket
		this.socket = null
		if (socket) socket.close(code, reason)
	}

	private clearTimers(): void {
		if (this.reconnectTimer) clearTimeout(this.reconnectTimer)
		if (this.expiryTimer) clearTimeout(this.expiryTimer)
		this.reconnectTimer = null
		this.expiryTimer = null
	}
}

function parseTicket(value: unknown): string | null {
	if (!isRecord(value) || typeof value.ticket !== 'string' || !/^[a-f0-9]{32}$/.test(value.ticket))
		return null
	return value.ticket
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
			return frame
		if (
			frame.type === 'presence.core' &&
			typeof frame.coreId === 'string' &&
			typeof frame.online === 'boolean' &&
			validHealth(frame.health) &&
			validDiagnostic(frame.diagnostic)
		)
			return frame
		if (frame.type === 'presence.snapshot' && validUsers(frame.users) && validCores(frame.cores))
			return frame
	} catch {
		return null
	}
	return null
}

function jwtExpiry(token: string): number | null {
	const parts = token.split('.')
	if (parts.length !== 3) return null
	try {
		const payload = JSON.parse(atob(parts[1].replace(/-/g, '+').replace(/_/g, '/'))) as {
			exp?: unknown
		}
		return typeof payload.exp === 'number' ? payload.exp * 1000 : null
	} catch {
		return null
	}
}

function validUsers(value: unknown): value is Record<string, boolean> {
	return isRecord(value) && Object.values(value).every((online) => typeof online === 'boolean')
}

function validCores(
	value: unknown,
): value is Record<
	string,
	{ online: boolean; health?: RealtimeCoreHealth; diagnostic?: RealtimeCoreDiagnostic }
> {
	return (
		isRecord(value) &&
		Object.values(value).every(
			(core) =>
				isRecord(core) &&
				typeof core.online === 'boolean' &&
				validHealth(core.health) &&
				validDiagnostic(core.diagnostic),
		)
	)
}

function validHealth(value: unknown): value is RealtimeCoreHealth | undefined {
	return value === undefined || value === 'healthy' || value === 'degraded' || value === 'offline'
}

function validDiagnostic(value: unknown): value is RealtimeCoreDiagnostic | undefined {
	return (
		value === undefined ||
		value === 'none' ||
		value === 'network' ||
		value === 'authentication' ||
		value === 'server'
	)
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null
}
