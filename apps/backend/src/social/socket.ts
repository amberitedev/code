import { DurableObject } from 'cloudflare:workers'
import { areFriends, requireSession } from '../common/auth'
import type { Env } from '../common/types'

type Presence = { user_id: string; profile_name: string | null; last_update: string }
type Connection = { userId: string; sessionId: string; expires: string; presence: Presence }

/** One object per account keeps all its installations online through Worker hibernation. */
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
		await this.ctx.storage.put('userId', session.user_id)
		await this.scheduleExpiry()
		const friends = await this.friendIds(session.user_id)
		const statuses = await Promise.all(friends.map((id) => this.env.FRIENDS.getByName(id).status()))
		pair[1].send(
			JSON.stringify({
				type: 'friend_statuses',
				statuses: statuses.filter((status) => status !== null),
			}),
		)
		await this.broadcast(session.user_id, { type: 'status_update', status: connection.presence })
		return new Response(null, { status: 101, webSocket: pair[0] })
	}

	async status(): Promise<Presence | null> {
		for (const socket of this.ctx.getWebSockets()) {
			if (socket.readyState !== WebSocket.OPEN) continue
			const connection = socket.deserializeAttachment() as Connection
			if (await this.valid(connection)) return connection.presence
			socket.close(4001, 'Session expired')
		}
		return null
	}

	async notify(message: unknown): Promise<void> {
		const text = JSON.stringify(message)
		for (const socket of this.ctx.getWebSockets()) {
			if (socket.readyState !== WebSocket.OPEN) continue
			if (await this.valid(socket.deserializeAttachment() as Connection)) socket.send(text)
			else socket.close(4001, 'Session expired')
		}
	}

	async receivePresence(from: string, message: unknown): Promise<void> {
		const userId = await this.ctx.storage.get<string>('userId')
		if (userId && (await areFriends(this.env, userId, from))) await this.notify(message)
	}

	async webSocketMessage(socket: WebSocket, raw: string | ArrayBuffer): Promise<void> {
		const connection = socket.deserializeAttachment() as Connection
		if (!(await this.valid(connection))) {
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
		const connection = socket.deserializeAttachment() as Connection
		socket.close()
		if (!(await this.status()))
			await this.broadcast(connection.userId, { type: 'user_offline', id: connection.userId })
	}

	async webSocketError(socket: WebSocket): Promise<void> {
		await this.webSocketClose(socket)
	}

	async alarm(): Promise<void> {
		const userId = await this.ctx.storage.get<string>('userId')
		if (userId && !(await this.status()))
			await this.broadcast(userId, { type: 'user_offline', id: userId })
		await this.scheduleExpiry()
	}

	/** Close revoked installations immediately, including idle hibernating sockets. */
	async revalidateSessions(): Promise<void> {
		const userId = await this.ctx.storage.get<string>('userId')
		if (userId && !(await this.status()))
			await this.broadcast(userId, { type: 'user_offline', id: userId })
	}

	private async valid(connection: Connection): Promise<boolean> {
		if (Date.parse(connection.expires) <= Date.now()) return false
		return Boolean(
			await this.env.DB.prepare(
				'SELECT id FROM sessions WHERE id = ? AND user_id = ? AND expires > ?',
			)
				.bind(connection.sessionId, connection.userId, new Date().toISOString())
				.first(),
		)
	}

	private async friendIds(userId: string): Promise<string[]> {
		const { results } = await this.env.DB.prepare(
			'SELECT CASE WHEN user_id = ? THEN friend_id ELSE user_id END AS id FROM friends WHERE accepted = 1 AND (user_id = ? OR friend_id = ?)',
		)
			.bind(userId, userId, userId)
			.all<{ id: string }>()
		return results.map((row) => row.id)
	}

	private async broadcast(userId: string, message: unknown): Promise<void> {
		await Promise.all(
			(await this.friendIds(userId)).map((id) =>
				this.env.FRIENDS.getByName(id).receivePresence(userId, message),
			),
		)
	}

	private async scheduleExpiry(): Promise<void> {
		const expiries = this.ctx
			.getWebSockets()
			.filter((socket) => socket.readyState === WebSocket.OPEN)
			.map((socket) => Date.parse((socket.deserializeAttachment() as Connection).expires))
			.filter((expires) => expires > Date.now())
		if (expiries.length) await this.ctx.storage.setAlarm(Math.min(...expiries))
	}
}

export async function notifyUser(env: Env, userId: string, message: unknown): Promise<void> {
	await env.FRIENDS.getByName(userId).notify(message)
}

export async function friendsStatuses(env: Env, userId: string): Promise<Presence[]> {
	const { results } = await env.DB.prepare(
		'SELECT CASE WHEN user_id = ? THEN friend_id ELSE user_id END AS id FROM friends WHERE accepted = 1 AND (user_id = ? OR friend_id = ?)',
	)
		.bind(userId, userId, userId)
		.all<{ id: string }>()
	const statuses = await Promise.all(results.map(({ id }) => env.FRIENDS.getByName(id).status()))
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
