import { DurableObject } from 'cloudflare:workers'
import { tokenFrom } from '../common/auth'
import { digest } from '../common/crypto'
import { ApiError, unauthorized } from '../common/http'
import type { Env } from '../common/types'

/**
 * Where a signed-out Core waits for its code to be approved, one object per Core. The Core holds a
 * WebSocket here instead of polling; approval sends `approved` and closes it, and the Core then
 * asks `/core/auth/poll` for the answer. Nothing else is sent, so the socket stays hibernated and
 * costs nothing while the Core waits.
 */
export class CoreSignIn extends DurableObject<Env> {
	fetch(): Response {
		const pair = new WebSocketPair()
		this.ctx.acceptWebSocket(pair[1])
		return new Response(null, { status: 101, webSocket: pair[0] })
	}

	/**
	 * The Core's code was approved: wake every socket it is waiting on. The message reaches it at
	 * once; a close alone can take seconds to arrive.
	 */
	approved(): void {
		for (const socket of this.ctx.getWebSockets()) {
			socket.send('approved')
			socket.close(1000, 'Approved')
		}
	}

	webSocketClose(socket: WebSocket): void {
		socket.close()
	}
}

/**
 * Core, signed out: wait for approval on a WebSocket, sent with the key from `requestCode`. 410
 * when the code is gone, like `/core/auth/poll`.
 */
export async function waitForApproval(request: Request, env: Env): Promise<Response> {
	if (request.headers.get('upgrade')?.toLowerCase() !== 'websocket')
		return new Response('WebSocket required', { status: 426 })
	const key = tokenFrom(request)
	if (!key.startsWith('amc_')) unauthorized()
	const pending = await env.DB.prepare(
		'SELECT core_id FROM core_auth_codes WHERE key_hash = ? AND expires > ?',
	)
		.bind(await digest(key), new Date().toISOString())
		.first<{ core_id: string }>()
	if (!pending) throw new ApiError(410, 'code_expired', 'That code has expired')
	return env.CORE_SIGN_IN.getByName(pending.core_id).fetch(request)
}
