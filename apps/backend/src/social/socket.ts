import type {
	ClientToServerMessage,
	FriendsSocketMessage,
	ServerToClientMessage,
	UserStatus,
} from '@amberite/contracts'
import { DurableObject } from 'cloudflare:workers'
import { requireSession } from '../common/auth'
import type { Env } from '../common/types'

/** What each socket keeps through hibernation. */
type Connection = { userId: string; sessionId: string; expires: string; presence: UserStatus }

/** Storage key of the friends this account knows are online, kept while it is online itself. */
const ONLINE_FRIENDS = 'online-friends'

/**
 * One object per account keeps all its installations online through Worker hibernation.
 *
 * A socket is trusted until its session's expiry, checked in memory. Revoking a session early
 * (signing out, a lock, an admin reset, deleting the account) calls `revalidateSessions`, so no
 * message or presence lookup has to ask D1 whether the session still exists.
 *
 * Coming online asks every friend's object, which answers with its presence. From then on the
 * object remembers which friends are online, from those answers and the presence messages it is
 * sent, so a status change or going offline wakes only the objects of friends who are online.
 */
export class FriendsHub extends DurableObject<Env> {
	/** The friends known to be online, as stored under `ONLINE_FRIENDS`; loaded on first use. */
	private known?: Set<string> | null

	async fetch(request: Request): Promise<Response> {
		const session = await requireSession(request, this.env)
		const wasOnline = this.online().length > 0
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
		// Another installation of an account already online asks only the friends known online.
		// Answers are merged, not assigned: a friend who connects while this call waits on D1 or
		// another object is added by `notify` and has no socket yet to answer with.
		if (!wasOnline) this.known = new Set()
		const statuses = (
			await this.broadcast(
				session.user_id,
				{ type: 'status_update', status: connection.presence },
				!wasOnline,
			)
		).filter((status) => status !== null)
		const known = new Set(await this.knownOnline())
		for (const status of statuses) known.add(status.user_id)
		await this.remember(known, !wasOnline)
		const sync: ServerToClientMessage = { type: 'friend_statuses', statuses }
		pair[1].send(JSON.stringify(sync))
		return new Response(null, { status: 101, webSocket: pair[0] })
	}

	/** This account's presence, or null when none of its installations is online. */
	status(): UserStatus | null {
		const [socket] = this.online()
		return socket ? attachment(socket).presence : null
	}

	/**
	 * Sends each message to every online installation of this account, and keeps the friends
	 * known online up to date from the presence messages among them.
	 */
	async notify(...messages: FriendsSocketMessage[]): Promise<void> {
		const sockets = this.online()
		if (!sockets.length) return
		const texts = messages.map((message) => JSON.stringify(message))
		for (const socket of sockets) for (const text of texts) socket.send(text)
		const changes = messages.flatMap((message): [string, boolean][] => {
			if (!('type' in message)) return []
			if (message.type === 'status_update') return [[message.status.user_id, true]]
			if (message.type === 'user_offline') return [[message.id, false]]
			if (message.type === 'friend_request_rejected') return [[message.from, false]]
			return []
		})
		if (!changes.length) return
		const current = await this.knownOnline()
		// Unknown stays unknown, so the next broadcast asks every friend instead of a partial set.
		if (!current) return
		const known = new Set(current)
		for (const [id, online] of changes)
			if (online) known.add(id)
			else known.delete(id)
		await this.remember(known)
	}

	/** A friend's presence changed: passes it on and answers with this account's own. */
	async friendPresence(message: ServerToClientMessage): Promise<UserStatus | null> {
		await this.notify(message)
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
		const update = statusUpdate(message)
		if (!update) return
		connection.presence = {
			user_id: connection.userId,
			profile_name: update.profile_name,
			last_update: new Date().toISOString(),
		}
		socket.serializeAttachment(connection)
		await this.broadcast(connection.userId, {
			type: 'status_update',
			status: connection.presence,
		})
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
		// A reconnect during the broadcast already rebuilt the set.
		if (this.online().length) return
		// Offline, it hears of no friend coming online, so what it knew goes stale.
		this.known = null
		await this.ctx.storage.delete(ONLINE_FRIENDS)
	}

	/**
	 * Sends a presence message to the friends known online, or with `everyone` (or nothing known)
	 * to every friend, and returns each one's own presence. Friendship is read from D1 each time,
	 * so a stale entry never reaches someone who is no longer a friend.
	 */
	private async broadcast(
		userId: string,
		message: ServerToClientMessage,
		everyone = false,
	): Promise<(UserStatus | null)[]> {
		const known = everyone ? null : await this.knownOnline()
		if (known && !known.size) return []
		const friends = await friendIds(this.env, userId)
		return Promise.all(
			(known ? friends.filter((id) => known.has(id)) : friends).map((id) =>
				this.env.FRIENDS.getByName(id).friendPresence(message),
			),
		)
	}

	private async knownOnline(): Promise<Set<string> | null> {
		if (this.known === undefined) {
			const stored = await this.ctx.storage.get<string[]>(ONLINE_FRIENDS)
			this.known = stored ? new Set(stored) : null
		}
		return this.known
	}

	/** `force` writes even an unchanged set, for one so far held only in memory. */
	private async remember(known: Set<string>, force = false): Promise<void> {
		const before = await this.knownOnline()
		if (!force && before && before.size === known.size && [...known].every((id) => before.has(id)))
			return
		this.known = known
		await this.ctx.storage.put(ONLINE_FRIENDS, [...known])
	}

	private async scheduleExpiry(): Promise<void> {
		const expiries = this.online().map((socket) => Date.parse(attachment(socket).expires))
		if (expiries.length) await this.ctx.storage.setAlarm(Math.min(...expiries))
	}
}

function attachment(socket: WebSocket): Connection {
	return socket.deserializeAttachment() as Connection
}

/**
 * `message` when it is a valid status update. The App's other messages tunnel sockets between
 * friends, which this backend does not do.
 */
function statusUpdate(
	message: unknown,
): Extract<ClientToServerMessage, { type: 'status_update' }> | null {
	if (
		message &&
		typeof message === 'object' &&
		'type' in message &&
		message.type === 'status_update' &&
		'profile_name' in message &&
		(message.profile_name === null ||
			(typeof message.profile_name === 'string' && message.profile_name.length <= 256))
	)
		return { type: 'status_update', profile_name: message.profile_name }
	return null
}

async function friendIds(env: Env, userId: string): Promise<string[]> {
	const { results } = await env.DB.prepare(
		'SELECT CASE WHEN user_id = ? THEN friend_id ELSE user_id END AS id FROM friends WHERE accepted = 1 AND (user_id = ? OR friend_id = ?)',
	)
		.bind(userId, userId, userId)
		.all<{ id: string }>()
	return results.map((row) => row.id)
}

export async function notifyUser(
	env: Env,
	userId: string,
	...messages: FriendsSocketMessage[]
): Promise<void> {
	await env.FRIENDS.getByName(userId).notify(...messages)
}

export async function friendsStatuses(env: Env, userId: string): Promise<UserStatus[]> {
	const statuses = await Promise.all(
		(await friendIds(env, userId)).map((id) => env.FRIENDS.getByName(id).status()),
	)
	return statuses.filter((status) => status !== null)
}

/** Hands the App's socket to its account's `FriendsHub`. */
export async function friendsSocket(request: Request, env: Env): Promise<Response> {
	if (request.headers.get('upgrade')?.toLowerCase() !== 'websocket')
		return new Response('WebSocket required', { status: 426 })
	const headers = new Headers(request.headers)
	headers.set('authorization', `Bearer ${new URL(request.url).searchParams.get('code') ?? ''}`)
	const authenticated = new Request(request, { headers })
	const session = await requireSession(authenticated, env)
	return env.FRIENDS.getByName(session.user_id).fetch(authenticated)
}
