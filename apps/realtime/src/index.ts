import { DurableObject } from 'cloudflare:workers'
import { createRemoteJWKSet, jwtVerify } from 'jose'

type ConnectionKind = 'desktop' | 'core'
type CoreHealth = 'healthy' | 'degraded' | 'offline'
type CoreDiagnostic = 'none' | 'network' | 'authentication' | 'server'
type Scope = {
	userId?: string
	friendUserIds?: string[]
	memberUserIds?: string[]
	coreId?: string | null
}
type Ticket = {
	kind: ConnectionKind
	id: string
	credentialHash?: string
	scope: Scope
	expiresAt: number
	jwtExpiresAt?: number
	sessionId: string
}
type Attachment = {
	kind: ConnectionKind
	id: string
	credentialHash?: string
	health?: CoreHealth
	diagnostic?: CoreDiagnostic
	expiresAt?: number
	sessionId: string
}

type RealtimeConfig = {
	CONVEX_JWKS_URL: string
	CONVEX_JWT_ISSUER: string
	CONVEX_JWT_AUDIENCE: string
	CONVEX_BRIDGE_URL: string
	REALTIME_BRIDGE_HMAC_SECRET: string
	DESKTOP_ORIGINS: string
}

const MAX_BODY_BYTES = 2048
const TICKET_TTL_MS = 60_000
const MAX_SESSION_ATTEMPTS_PER_MINUTE = 20
const MAX_INVALIDATION_IDENTITIES = 100
let jwksUrl: string | null = null
let jwks: ReturnType<typeof createRemoteJWKSet> | null = null

export default {
	async fetch(request: Request, env: Env): Promise<Response> {
		const url = new URL(request.url)
		if (request.method === 'OPTIONS') return cors(new Response(null, { status: 204 }), request, env)
		if (url.pathname === '/v1/desktop-sessions' && request.method === 'POST')
			return cors(await desktopSession(request, env), request, env)
		if (url.pathname === '/v1/core-sessions' && request.method === 'POST')
			return cors(await coreSession(request, env), request, env)
		if (url.pathname === '/v1/connect' && request.method === 'GET') return connect(request, env)
		if (url.pathname === '/v1/invalidate' && request.method === 'POST')
			return invalidate(request, env)
		return new Response('Not found', { status: 404 })
	},
}

export class PresenceHub extends DurableObject<Env> {
	async issueTicket(ticket: Ticket, clientAddress: string): Promise<string | null> {
		const key = `rate:${clientAddress}`
		const rate = await this.ctx.storage.get<{ count: number; expiresAt: number }>(key)
		const now = Date.now()
		if (rate && rate.expiresAt > now && rate.count >= MAX_SESSION_ATTEMPTS_PER_MINUTE) return null
		await this.ctx.storage.put(key, {
			count: rate && rate.expiresAt > now ? rate.count + 1 : 1,
			expiresAt: now + 60_000,
		})
		const token = crypto.randomUUID().replace(/-/g, '')
		await this.ctx.storage.put(`ticket:${token}`, ticket)
		await this.ctx.storage.setAlarm(now + TICKET_TTL_MS)
		return token
	}

	async consumeTicket(token: string): Promise<Ticket | null> {
		const key = `ticket:${token}`
		const ticket = await this.ctx.storage.get<Ticket>(key)
		if (!ticket || ticket.expiresAt <= Date.now()) {
			await this.ctx.storage.delete(key)
			return null
		}
		await this.ctx.storage.delete(key)
		return ticket
	}

	async accept(request: Request, ticket: Ticket): Promise<Response> {
		const pair = new WebSocketPair()
		const [client, server] = Object.values(pair)
		const attachment: Attachment = {
			kind: ticket.kind,
			id: ticket.id,
			credentialHash: ticket.credentialHash,
			expiresAt: ticket.jwtExpiresAt,
			sessionId: ticket.sessionId,
		}
		server.serializeAttachment(attachment)
		this.ctx.acceptWebSocket(server, [tag(ticket.kind, ticket.id), sessionTag(ticket.sessionId)])
		if (ticket.jwtExpiresAt) {
			await this.ctx.storage.put(`session:${ticket.sessionId}`, { expiresAt: ticket.jwtExpiresAt })
			await this.ctx.storage.setAlarm(ticket.jwtExpiresAt)
		}
		await this.publishLifecycle(attachment, true)
		await this.sendSnapshot(server, ticket)
		return new Response(null, { status: 101, webSocket: client })
	}

	async webSocketMessage(socket: WebSocket, message: string | ArrayBuffer): Promise<void> {
		if (typeof message !== 'string' || message.length > MAX_BODY_BYTES)
			return socket.close(1009, 'Message too large')
		const attachment = socket.deserializeAttachment() as Attachment | null
		if (attachment?.expiresAt && attachment.expiresAt <= Date.now())
			return socket.close(4001, 'Authorization expired')
		if (!attachment || attachment.kind !== 'core') return socket.close(1008, 'Unsupported message')
		let frame: { type?: unknown; health?: unknown; diagnostic?: unknown }
		try {
			frame = JSON.parse(message)
		} catch {
			return socket.close(1008, 'Invalid message')
		}
		if (frame.type !== 'core.health' || !isHealth(frame.health) || !isDiagnostic(frame.diagnostic))
			return socket.close(1008, 'Invalid health frame')
		if (attachment.health === frame.health && attachment.diagnostic === frame.diagnostic) return
		attachment.health = frame.health
		attachment.diagnostic = frame.diagnostic
		socket.serializeAttachment(attachment)
		await this.publishCore(attachment)
	}

	async webSocketClose(socket: WebSocket): Promise<void> {
		const attachment = socket.deserializeAttachment() as Attachment | null
		if (attachment) {
			await this.ctx.storage.delete(`session:${attachment.sessionId}`)
			await this.publishLifecycle(attachment, false)
		}
	}

	async webSocketError(socket: WebSocket): Promise<void> {
		const attachment = socket.deserializeAttachment() as Attachment | null
		if (attachment) {
			await this.ctx.storage.delete(`session:${attachment.sessionId}`)
			await this.publishLifecycle(attachment, false)
		}
	}

	async alarm(): Promise<void> {
		const now = Date.now()
		for (const entry of await this.ctx.storage.list<Ticket>({ prefix: 'ticket:', limit: 100 }))
			if (entry[1].expiresAt <= now) await this.ctx.storage.delete(entry[0])
		for (const entry of await this.ctx.storage.list<{ expiresAt: number }>({
			prefix: 'rate:',
			limit: 100,
		}))
			if (entry[1].expiresAt <= now) await this.ctx.storage.delete(entry[0])
		for (const entry of await this.ctx.storage.list<{ expiresAt: number }>({
			prefix: 'session:',
			limit: 100,
		})) {
			if (entry[1].expiresAt > now) continue
			for (const socket of this.ctx.getWebSockets(sessionTag(entry[0].slice('session:'.length))))
				socket.close(4001, 'Authorization expired')
			await this.ctx.storage.delete(entry[0])
		}
	}

	async invalidate(input: { userIds: string[]; coreIds: string[] }): Promise<void> {
		for (const userId of input.userIds) {
			for (const socket of this.ctx.getWebSockets(tag('desktop', userId))) {
				socket.send(JSON.stringify({ type: 'authorization.invalidated' }))
				socket.close(4003, 'Authorization invalidated')
			}
		}
		for (const coreId of input.coreIds)
			for (const socket of this.ctx.getWebSockets(tag('core', coreId)))
				socket.close(4003, 'Authorization invalidated')
	}

	private async publishLifecycle(attachment: Attachment, connected: boolean): Promise<void> {
		const sockets = this.ctx.getWebSockets(tag(attachment.kind, attachment.id))
		const online = connected ? sockets.length === 1 : sockets.length === 0
		if (!online) return
		const scope = await bridge(
			this.env,
			attachment.kind === 'desktop'
				? { operation: 'recipients', kind: 'desktop', id: attachment.id }
				: {
						operation: 'recipients',
						kind: 'core',
						id: attachment.id,
						credentialHash: attachment.credentialHash,
					},
		)
		if (!scope) return
		const recipients =
			attachment.kind === 'desktop' ? (scope.friendUserIds ?? []) : (scope.memberUserIds ?? [])
		const frame =
			attachment.kind === 'desktop'
				? { type: 'presence.user', userId: attachment.id, online: connected }
				: {
						type: 'presence.core',
						coreId: attachment.id,
						online: connected,
						health: attachment.health ?? 'offline',
						diagnostic: attachment.diagnostic ?? 'none',
					}
		this.sendUsers(recipients, frame)
	}

	private async publishCore(attachment: Attachment): Promise<void> {
		const scope = await bridge(this.env, {
			operation: 'recipients',
			kind: 'core',
			id: attachment.id,
			credentialHash: attachment.credentialHash,
		})
		if (!scope) return
		this.sendUsers(scope.memberUserIds ?? [], {
			type: 'presence.core',
			coreId: attachment.id,
			online: this.ctx.getWebSockets(tag('core', attachment.id)).length > 0,
			health: attachment.health,
			diagnostic: attachment.diagnostic,
		})
	}

	private async sendSnapshot(socket: WebSocket, ticket: Ticket): Promise<void> {
		const users = Object.fromEntries(
			(ticket.scope.friendUserIds ?? []).map((id) => [
				id,
				this.ctx.getWebSockets(tag('desktop', id)).length > 0,
			]),
		)
		const coreId = ticket.scope.coreId
		const cores = coreId ? { [coreId]: this.coreSnapshot(coreId) } : {}
		socket.send(JSON.stringify({ type: 'presence.snapshot', users, cores }))
	}

	private coreSnapshot(coreId: string): {
		online: boolean
		health: CoreHealth
		diagnostic: CoreDiagnostic
	} {
		const core = this.ctx.getWebSockets(tag('core', coreId))[0]
		const attachment = core?.deserializeAttachment() as Attachment | null
		return {
			online: !!core,
			health: attachment?.health ?? 'offline',
			diagnostic: attachment?.diagnostic ?? 'none',
		}
	}

	private sendUsers(userIds: string[], frame: unknown): void {
		const payload = JSON.stringify(frame)
		for (const userId of new Set(userIds))
			for (const socket of this.ctx.getWebSockets(tag('desktop', userId))) socket.send(payload)
	}
}

async function desktopSession(request: Request, env: Env): Promise<Response> {
	const origin = request.headers.get('origin')
	if (!origin || !allowedOrigin(origin, env)) return new Response('Forbidden', { status: 403 })
	const token = bearer(request)
	if (!token) return new Response('Unauthorized', { status: 401 })
	const payload = await verifyDesktopJwt(token, env).catch(() => null)
	const userId = typeof payload?.sub === 'string' ? payload.sub : null
	if (!userId) return new Response('Unauthorized', { status: 401 })
	const scope = await bridge(env, { operation: 'desktopScope', userId })
	if (!scope) return new Response('Forbidden', { status: 403 })
	const jwtExpiresAt = typeof payload.exp === 'number' ? payload.exp * 1000 : null
	if (!jwtExpiresAt || jwtExpiresAt <= Date.now())
		return new Response('Unauthorized', { status: 401 })
	const tokenValue = await hub(env).issueTicket(
		{
			kind: 'desktop',
			id: userId,
			scope,
			expiresAt: Math.min(Date.now() + TICKET_TTL_MS, jwtExpiresAt),
			jwtExpiresAt,
			sessionId: crypto.randomUUID(),
		},
		clientAddress(request),
	)
	return tokenValue
		? Response.json({ ticket: tokenValue, expiresAt: Date.now() + TICKET_TTL_MS })
		: new Response('Too many requests', { status: 429 })
}

async function coreSession(request: Request, env: Env): Promise<Response> {
	const input = await boundedJson<{ coreId?: string; credential?: string }>(request)
	if (!input?.coreId || !input.credential || input.credential.length !== 64)
		return new Response('Unauthorized', { status: 401 })
	const credentialHash = await sha256(input.credential)
	const scope = await bridge(env, { operation: 'coreScope', coreId: input.coreId, credentialHash })
	if (!scope) return new Response('Unauthorized', { status: 401 })
	const tokenValue = await hub(env).issueTicket(
		{
			kind: 'core',
			id: input.coreId,
			credentialHash,
			scope,
			expiresAt: Date.now() + TICKET_TTL_MS,
			sessionId: crypto.randomUUID(),
		},
		clientAddress(request),
	)
	return tokenValue
		? Response.json({ ticket: tokenValue, expiresAt: Date.now() + TICKET_TTL_MS })
		: new Response('Too many requests', { status: 429 })
}

async function connect(request: Request, env: Env): Promise<Response> {
	if (request.headers.get('upgrade')?.toLowerCase() !== 'websocket')
		return new Response('Upgrade required', { status: 426 })
	const token = new URL(request.url).searchParams.get('ticket')
	if (!token || !/^[a-f0-9]{32}$/.test(token)) return new Response('Unauthorized', { status: 401 })
	const ticket = await hub(env).consumeTicket(token)
	return ticket ? hub(env).accept(request, ticket) : new Response('Unauthorized', { status: 401 })
}

async function invalidate(request: Request, env: Env): Promise<Response> {
	const input = await signedJson<{ userIds?: unknown; coreIds?: unknown }>(request, env)
	if (!input) return new Response('Unauthorized', { status: 401 })
	const userIds = validIds(input.userIds)
	const coreIds = validIds(input.coreIds)
	if (!userIds || !coreIds) return new Response('Bad request', { status: 400 })
	await hub(env).invalidate({ userIds, coreIds })
	return Response.json({ ok: true })
}

function hub(env: Env) {
	return env.PRESENCE_HUB.getByName('global-v1')
}
function tag(kind: ConnectionKind, id: string) {
	return `${kind}:${id}`
}
function sessionTag(sessionId: string) {
	return `session:${sessionId}`
}
function bearer(request: Request) {
	const value = request.headers.get('authorization')
	return value?.startsWith('Bearer ') ? value.slice(7) : null
}
function clientAddress(request: Request) {
	return request.headers.get('CF-Connecting-IP') ?? 'unknown'
}
function isHealth(value: unknown): value is CoreHealth {
	return value === 'healthy' || value === 'degraded' || value === 'offline'
}
function isDiagnostic(value: unknown): value is CoreDiagnostic {
	return value === 'none' || value === 'network' || value === 'authentication' || value === 'server'
}
function allowedOrigin(origin: string, env: Env) {
	return config(env)
		.DESKTOP_ORIGINS.split(',')
		.map((value) => value.trim())
		.filter(Boolean)
		.includes(origin)
}
function cors(response: Response, request: Request, env: Env) {
	const origin = request.headers.get('origin')
	if (!origin || !allowedOrigin(origin, env)) return response
	const headers = new Headers(response.headers)
	headers.set('Access-Control-Allow-Origin', origin)
	headers.set('Access-Control-Allow-Headers', 'Authorization, Content-Type')
	headers.set('Access-Control-Allow-Methods', 'POST, OPTIONS')
	return new Response(response.body, { status: response.status, headers })
}
async function boundedJson<T>(request: Request): Promise<T | null> {
	const length = Number(request.headers.get('content-length') ?? 0)
	if (length > MAX_BODY_BYTES) return null
	const body = await request.text()
	if (body.length > MAX_BODY_BYTES) return null
	try {
		return JSON.parse(body) as T
	} catch {
		return null
	}
}
async function verifyDesktopJwt(token: string, env: Env) {
	const runtime = config(env)
	if (jwksUrl !== runtime.CONVEX_JWKS_URL) {
		jwksUrl = runtime.CONVEX_JWKS_URL
		jwks = createRemoteJWKSet(new URL(runtime.CONVEX_JWKS_URL))
	}
	return (
		await jwtVerify(token, jwks!, {
			issuer: runtime.CONVEX_JWT_ISSUER,
			audience: runtime.CONVEX_JWT_AUDIENCE,
		})
	).payload
}
async function bridge(env: Env, body: Record<string, unknown>): Promise<Scope | null> {
	const runtime = config(env)
	const serialized = JSON.stringify(body)
	const timestamp = String(Date.now())
	const signature = await hmac(`${timestamp}.${serialized}`, runtime.REALTIME_BRIDGE_HMAC_SECRET)
	const response = await fetch(runtime.CONVEX_BRIDGE_URL, {
		method: 'POST',
		headers: {
			'content-type': 'application/json',
			'x-amberite-timestamp': timestamp,
			'x-amberite-signature': signature,
		},
		body: serialized,
	})
	return response.ok ? ((await response.json()) as Scope) : null
}
async function hmac(value: string, secret: string) {
	const key = await crypto.subtle.importKey(
		'raw',
		new TextEncoder().encode(secret),
		{ name: 'HMAC', hash: 'SHA-256' },
		false,
		['sign'],
	)
	return base64Url(
		new Uint8Array(await crypto.subtle.sign('HMAC', key, new TextEncoder().encode(value))),
	)
}
async function sha256(value: string) {
	return base64Url(
		new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(value))),
	)
}
function base64Url(bytes: Uint8Array) {
	let value = ''
	for (const byte of bytes) value += String.fromCharCode(byte)
	return btoa(value).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '')
}
function config(env: Env): RealtimeConfig {
	const runtime = env as Env & Partial<RealtimeConfig>
	const keys: Array<keyof RealtimeConfig> = [
		'CONVEX_JWKS_URL',
		'CONVEX_JWT_ISSUER',
		'CONVEX_JWT_AUDIENCE',
		'CONVEX_BRIDGE_URL',
		'REALTIME_BRIDGE_HMAC_SECRET',
		'DESKTOP_ORIGINS',
	]
	for (const key of keys) if (!runtime[key]) throw new Error(`${key} must be configured`)
	return runtime as RealtimeConfig
}
async function signedJson<T>(request: Request, env: Env): Promise<T | null> {
	const timestamp = request.headers.get('x-amberite-timestamp')
	const signature = request.headers.get('x-amberite-signature')
	if (!timestamp || !signature || Math.abs(Date.now() - Number(timestamp)) > 30_000) return null
	const length = Number(request.headers.get('content-length') ?? 0)
	if (length > MAX_BODY_BYTES) return null
	const body = await request.text()
	if (
		body.length > MAX_BODY_BYTES ||
		!(await validHmac(`${timestamp}.${body}`, signature, config(env).REALTIME_BRIDGE_HMAC_SECRET))
	)
		return null
	try {
		return JSON.parse(body) as T
	} catch {
		return null
	}
}
function validIds(value: unknown): string[] | null {
	return Array.isArray(value) &&
		value.length <= MAX_INVALIDATION_IDENTITIES &&
		value.every((id) => typeof id === 'string' && id.length > 0 && id.length <= 256)
		? value
		: null
}
async function validHmac(value: string, signature: string, secret: string): Promise<boolean> {
	const expected = new TextEncoder().encode(await hmac(value, secret))
	const received = new TextEncoder().encode(signature)
	if (expected.length !== received.length) return false
	let difference = 0
	for (let index = 0; index < expected.length; index++)
		difference |= expected[index] ^ received[index]
	return difference === 0
}
