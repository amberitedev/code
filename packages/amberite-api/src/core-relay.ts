import type { PlatformAdapter } from './adapter'
import { CoreOfflineError, NetworkError } from './errors'
import type { MessageEnvelope } from './transport'

export async function publishCoreRelay<TPayload>(
	adapter: PlatformAdapter,
	envelope: MessageEnvelope<TPayload>,
): Promise<void> {
	await coreRelayCall(adapter, '/relay/messages', {
		method: 'POST',
		body: JSON.stringify({
			type: envelope.type,
			version: envelope.version,
			sender_id: envelope.senderId,
			recipient_id: envelope.recipientId,
			payload: envelope.payload,
			ack: envelope.ack,
			ttl_ms: envelope.ttlMs,
		}),
	})
}

export async function pendingCoreRelayMessages(
	adapter: PlatformAdapter,
	recipientId: string,
): Promise<unknown[]> {
	const body = await coreRelayCall<{ messages: unknown[] }>(
		adapter,
		`/relay/messages/${encodeURIComponent(recipientId)}`,
		{
			method: 'GET',
		},
	)
	return body.messages
}

export async function writeCoreRelayReceipt(
	adapter: PlatformAdapter,
	messageId: string,
	recipientId: string,
): Promise<void> {
	await coreRelayCall(adapter, `/relay/messages/${encodeURIComponent(messageId)}/ack`, {
		method: 'POST',
		body: JSON.stringify({ recipient_id: recipientId }),
	})
}

export async function writeCoreRelayResult(
	adapter: PlatformAdapter,
	messageId: string,
	recipientId: string,
	result: unknown,
): Promise<void> {
	await coreRelayCall(adapter, `/relay/messages/${encodeURIComponent(messageId)}/complete`, {
		method: 'POST',
		body: JSON.stringify({ recipient_id: recipientId, result }),
	})
}

async function coreRelayCall<T = unknown>(
	adapter: PlatformAdapter,
	path: string,
	init: RequestInit,
): Promise<T> {
	const coreUrl = await adapter.getCoreUrl()
	if (!coreUrl) throw new CoreOfflineError()
	const token = await adapter.getCurrentJwt()
	const controller = new AbortController()
	const timeout = setTimeout(() => controller.abort(), 15_000)
	let res: Response
	try {
		res = await adapter.fetchFn(`${coreUrl}${path}`, {
			...init,
			signal: controller.signal,
			headers: {
				'Content-Type': 'application/json',
				...(token ? { Authorization: `Bearer ${token}` } : {}),
				...init.headers,
			},
		})
	} catch (error) {
		const reason = error instanceof Error ? error.message : String(error)
		throw new NetworkError(reason)
	} finally {
		clearTimeout(timeout)
	}
	const body = await res.json().catch(() => null)
	if (!res.ok) throw new NetworkError(body?.error ?? `Core relay failed: ${res.status}`)
	return body as T
}
