import type { Dialer, Introduced, Introduction, Listener } from '@amberite/contracts/backend'
import { DurableObject } from 'cloudflare:workers'

import { requireUser } from '../common/auth'
import { ApiError, invalid, objectBody, stringField } from '../common/http'
import type { Env } from '../common/types'

/** How long a Core may take to answer an introduction. It asks a STUN server first. */
const ANSWER_MS = 5000
/** The most addresses either side may offer, as `connect::MAX_CANDIDATES`. */
const MAX_CANDIDATES = 8
const ID = /^[0-9a-f]{64}$/
/** `ip:port`, an IPv6 address in brackets. The Core parses it for real. */
const CANDIDATE = /^(\d{1,3}(\.\d{1,3}){3}|\[[0-9a-f:.]+\]):\d{1,5}$/i

/**
 * Where a signed-in Core waits to be introduced, one object per Core. The Core holds a WebSocket
 * here for as long as it runs. Nothing is sent on it until someone wants to connect, and the
 * runtime answers the Core's pings itself, so the object stays hibernated and a waiting Core costs
 * nothing. An introduction is one message each way, and no address is ever stored.
 */
export class CoreLink extends DurableObject<Env> {
	/** Introductions the Core has not answered yet. Lost with the object, and so are their callers. */
	private waiting = new Map<string, (answer: Listener | null) => void>()

	/** The Core connects. An older socket of the same Core is one it lost, and goes. */
	fetch(): Response {
		for (const old of this.ctx.getWebSockets()) old.close(1000, 'Replaced')
		const pair = new WebSocketPair()
		this.ctx.acceptWebSocket(pair[1])
		return new Response(null, { status: 101, webSocket: pair[0] })
	}

	/** Tell the Core about `dialer` and answer with where the Core is, or null when it is off. */
	async introduce(dialer: Dialer): Promise<Listener | null> {
		const sockets = this.ctx.getWebSockets()
		if (!sockets.length) return null
		const request = crypto.randomUUID()
		const introduction: Introduction = { request, dialer }
		let giveUp: ReturnType<typeof setTimeout> | undefined
		const answered = new Promise<Listener | null>((resolve) => {
			this.waiting.set(request, resolve)
			giveUp = setTimeout(() => resolve(null), ANSWER_MS)
		})
		try {
			// A socket the Core replaced may still be closing; sending on it throws.
			let sent = 0
			for (const socket of sockets) {
				try {
					socket.send(JSON.stringify(introduction))
					sent++
				} catch {
					continue
				}
			}
			return sent ? await answered : null
		} finally {
			clearTimeout(giveUp)
			this.waiting.delete(request)
		}
	}

	webSocketMessage(_socket: WebSocket, raw: string | ArrayBuffer): void {
		if (typeof raw !== 'string') return
		let answer: Introduced
		try {
			answer = parseIntroduced(JSON.parse(raw))
		} catch {
			return
		}
		this.waiting.get(answer.request)?.({ candidates: answer.candidates })
	}

	/** The Core was removed from its account: drop its link, so it learns at once. */
	unlink(): void {
		for (const socket of this.ctx.getWebSockets()) socket.close(1000, 'Removed')
		for (const answer of this.waiting.values()) answer(null)
	}

	webSocketClose(socket: WebSocket): void {
		this.gone(socket)
		socket.close()
	}

	webSocketError(socket: WebSocket): void {
		this.gone(socket)
	}

	/** Nobody is left to answer once the Core's last socket is gone. */
	private gone(lost: WebSocket): void {
		const open = this.ctx
			.getWebSockets()
			.some((socket) => socket !== lost && socket.readyState === WebSocket.OPEN)
		if (open) return
		for (const answer of this.waiting.values()) answer(null)
	}
}

function candidates(body: Record<string, unknown>): string[] {
	const list = body.candidates
	if (
		!Array.isArray(list) ||
		list.length > MAX_CANDIDATES ||
		!list.every((entry): entry is string => typeof entry === 'string' && CANDIDATE.test(entry))
	)
		invalid('Invalid candidates')
	return list
}

function parseIntroduced(raw: unknown): Introduced {
	const body = objectBody(raw)
	return { request: stringField(body, 'request', 64), candidates: candidates(body) }
}

export function parseDialer(raw: unknown): Dialer {
	const body = objectBody(raw)
	const id = stringField(body, 'id', 64)
	if (!ID.test(id)) invalid('Invalid id')
	return { id, candidates: candidates(body) }
}

/**
 * App: introduce this device to one of the account's Cores. Only the owner may; anyone else gets
 * the same 404 as for a Core that does not exist. Nothing is written.
 */
export async function introduce(
	request: Request,
	env: Env,
	coreId: string,
	body: () => Promise<Dialer>,
): Promise<Listener> {
	const user = await requireUser(request, env)
	const allowed = await env.DB.prepare('SELECT 1 FROM cores WHERE id = ? AND owner_id = ?')
		.bind(coreId, user.id)
		.first()
	if (!allowed) throw new ApiError(404, 'not_found', 'Core not found')
	const listener = await env.CORE_LINK.getByName(coreId).introduce(await body())
	if (!listener)
		throw new ApiError(503, 'core_offline', 'This Core is not connected to Amberite right now.')
	return listener
}
