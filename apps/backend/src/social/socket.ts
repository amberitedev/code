import { DurableObject } from 'cloudflare:workers'
import { requireSession } from '../common/auth'
import type { Env } from '../common/types'

type Presence = { user_id: string; profile_name: string | null; last_update: string }
type Connection = { userId: string; sessionId: string; expires: string; presence: Presence }

/**
 * One object per account keeps all its installations online through Worker hibernation.
 *
 * A socket is trusted until its session's expiry, checked in memory. Revoking a session early
 * (signing out, a lock, an admin reset, deleting the account) calls `revalidateSessions`, so no
 * message or presence lookup has to ask D1 whether the session still exists.
 */
export class FriendsHub extends DurableObject<Env> {
	async fetch(request: Request): Promise<Response> {
		const session = await requireSession(request, this.env)
		const pair = new WebSocketPair()
		const connection: Connection = {
			userId: session.user_id,
			sessionId: session.id,
			expires: session.expires,
			presence: {
				user_id: session.user_id,
				profile_name: null,
				last_update: new Date().toISOString(),
			},
		}
		pair[1].serializeAttachment(connection)
		this.ctx.acceptWebSocket(pair[1])
		await this.scheduleExpiry()
		// The same call that tells each friend this account is online answers with their presence.
		const statuses = await this.broadcast(session.user_id, {
			type: 'status_update',
			status: connection.presence,
		})
		pair[1].send(
			JSON.stringify({
				type: 'friend_statuses',
				statuses: statuses.filter((status) => status !== null),
			}),
		)
		return new Response(null, { status: 101, webSocket: pair[0] })
	}

	/** This account's presence, or null when none of its installations is online. */
	status(): Presence | null {
		const [socket] = this.online()
		return socket ? attachment(socket).presence : null
	}

	/** Sends each message to every online installation of this account. */
	notify(...messages: unknown[]): void {
		const texts = messages.map((message) => JSON.stringify(message))
		for (const socket of this.online()) for (const text of texts) socket.send(text)
	}

	/** A friend's presence changed: passes it on and answers with this account's own. */
	friendPresence(message: unknown): Presence | null {
		this.notify(message)
		return this.status()
	}

	async webSocketMessage(socket: WebSocket, raw: string | ArrayBuffer): Promise<void> {
		const connection = attachment(socket)
		if (Date.parse(connection.expires) <= Date.now()) {
			socket.close(4001, 'Session expired')
			return
		}
		if (typeof raw !== 'string' || raw.length > 4096) {
			socket.close(1009, 'Invalid message')
			return
		}
		let message: unknown
		try {
			message = JSON.parse(raw)
		} catch {
			socket.close(1007, 'Invalid JSON')
			return
		}
		if (!message || typeof message !== 'object' || !('type' in message)) return
		if (
			message.type === 'status_update' &&
			'profile_name' in message &&
			(message.profile_name === null ||
				(typeof message.profile_name === 'string' && message.profile_name.length <= 256))
		) {
			connection.presence = {
				user_id: connection.userId,
				profile_name: message.profile_name,
				last_update: new Date().toISOString(),
			}
			socket.serializeAttachment(connection)
			await this.broadcast(connection.userId, {
				type: 'status_update',
				status: connection.presence,
			})
		}
	}

	async webSocketClose(socket: WebSocket): Promise<void> {
		const connection = attachment(socket)
		socket.close()
		if (!this.status()) await this.wentOffline(connection.userId)
	}

	async webSocketError(socket: WebSocket): Promise<void> {
		await this.webSocketClose(socket)
	}

	/** Set for the earliest session expiry: closes the installations whose session ended. */
	async alarm(): Promise<void> {
		const now = Date.now()
		const expired = this.ctx
			.getWebSockets()
			.filter(
				(socket) =>
					socket.readyState === WebSocket.OPEN && Date.parse(attachment(socket).expires) <= now,
			)
		for (const socket of expired) socket.close(4001, 'Session expired')
		if (expired[0] && !this.status()) await this.wentOffline(attachment(expired[0]).userId)
		await this.scheduleExpiry()
	}

	/** Closes the installations whose session was revoked, including idle hibernating sockets. */
	async revalidateSessions(): Promise<void> {
		const sockets = this.online()
		if (!sockets.length) return
		const { results } = await this.env.DB.prepare(
			'SELECT id FROM sessions WHERE id IN (SELECT value FROM json_each(?)) AND expires > ?',
		)
			.bind(
				JSON.stringify(sockets.map((socket) => attachment(socket).sessionId)),
				new Date().toISOString(),
			)
			.all<{ id: string }>()
		const live = new Set(results.map((row) => row.id))
		const revoked = sockets.filter((socket) => !live.has(attachment(socket).sessionId))
		for (const socket of revoked) socket.close(4001, 'Session expired')
		if (revoked[0] && !this.status()) await this.wentOffline(attachment(revoked[0]).userId)
	}

	/** Open sockets whose session has not expired. */
	private online(): WebSocket[] {
		const now = Date.now()
		return this.ctx
			.getWebSockets()
			.filter(
				(socket) =>
					socket.readyState === WebSocket.OPEN && Date.parse(attachment(socket).expires) > now,
			)
	}

	private async wentOffline(userId: string): Promise<void> {
		await this.broadcast(userId, { type: 'user_offline', id: userId })
	}

	/** Sends a presence message to every friend, and returns each friend's own presence. */
	private async broadcast(userId: string, message: unknown): Promise<(Presence | null)[]> {
		return Promise.all(
			(await friendIds(this.env, userId)).map((id) =>
				this.env.FRIENDS.getByName(id).friendPresence(message),
			),
		)
	}

	private async scheduleExpiry(): Promise<void> {
		const expiries = this.online().map((socket) => Date.parse(attachment(socket).expires))
		if (expiries.length) await this.ctx.storage.setAlarm(Math.min(...expiries))
	}
}

function attachment(socket: WebSocket): Connection {
	return socket.deserializeAttachment() as Connection
}

async function friendIds(env: Env, userId: string): Promise<string[]> {
	const { results } = await env.DB.prepare(
		'SELECT CASE WHEN user_id = ? THEN friend_id ELSE user_id END AS id FROM friends WHERE accepted = 1 AND (user_id = ? OR friend_id = ?)',
	)
		.bind(userId, userId, userId)
		.all<{ id: string }>()
	return results.map((row) => row.id)
}

export async function notifyUser(env: Env, userId: string, ...messages: unknown[]): Promise<void> {
	await env.FRIENDS.getByName(userId).notify(...messages)
}

export async function friendsStatuses(env: Env, userId: string): Promise<Presence[]> {
	const statuses = await Promise.all(
		(await friendIds(env, userId)).map((id) => env.FRIENDS.getByName(id).status()),
	)
	return statuses.filter((status) => status !== null)
}

export async function handleFriendsSocket(request: Request, env: Env): Promise<Response | null> {
	if (
		!['/_internal/launcher_socket', '/v2/_internal/launcher_socket'].includes(
			new URL(request.url).pathname,
		)
	)
		return null
	if (request.headers.get('upgrade')?.toLowerCase() !== 'websocket')
		return new Response('WebSocket required', { status: 426 })
	const headers = new Headers(request.headers)
	headers.set('authorization', `Bearer ${new URL(request.url).searchParams.get('code') ?? ''}`)
	const authenticated = new Request(request, { headers })
	const session = await requireSession(authenticated, env)
	return env.FRIENDS.getByName(session.user_id).fetch(authenticated)
}
