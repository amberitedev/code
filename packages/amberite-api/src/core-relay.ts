import type { PlatformAdapter } from './adapter'
import { verifyCoreConnection } from './connection'
import { CoreApiError, CoreOfflineError, NetworkError, RelayTimeoutError } from './errors'
import type { MessageEnvelope } from './transport'

const CORE_RELAY_POLL_INTERVAL_MS = 500

export async function publishCoreRelay<TPayload>(
	adapter: PlatformAdapter,
	envelope: MessageEnvelope<TPayload>,
): Promise<void> {
	await coreRelayCall(adapter, '/relay/messages', {
		method: 'POST',
		body: JSON.stringify({
			id: envelope.id,
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

export async function coreRelayMessageStatus(
	adapter: PlatformAdapter,
	messageId: string,
): Promise<unknown | null> {
	const body = await coreRelayCall<{ message: unknown | null }>(
		adapter,
		`/relay/messages/status/${encodeURIComponent(messageId)}`,
		{
			method: 'GET',
		},
	)
	return body.message
}

export async function waitForCoreRelayReceipt(
	adapter: PlatformAdapter,
	messageId: string,
	timeoutMs: number,
): Promise<void> {
	await waitForCoreRelayStatus(adapter, messageId, timeoutMs, ['received', 'processed'])
}

export async function waitForCoreRelayResult(
	adapter: PlatformAdapter,
	messageId: string,
	timeoutMs: number,
): Promise<unknown> {
	const status = await waitForCoreRelayStatus(adapter, messageId, timeoutMs, ['processed'])
	if (status?.error) throw new NetworkError(String(status.error))
	return status?.result
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
	const knownCoreId = await adapter.getConnectedCoreId?.()
	if (knownCoreId) {
		const status = await verifyCoreConnection(adapter, { knownCoreId })
		if (
			status.state !== 'connected' ||
			status.coreUrl !== coreUrl ||
			status.coreId !== knownCoreId
		) {
			throw new CoreOfflineError()
		}
	}
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
		if (error instanceof Error && error.name === 'AbortError') {
			throw new RelayTimeoutError()
		}
		const reason = error instanceof Error ? error.message : String(error)
		throw new NetworkError(reason)
	} finally {
		clearTimeout(timeout)
	}
	const body = await res.json().catch(() => null)
	if (!res.ok) throw new CoreApiError(res.status, body?.error ?? 'Core relay request failed')
	return body as T
}

async function waitForCoreRelayStatus(
	adapter: PlatformAdapter,
	messageId: string,
	timeoutMs: number,
	statuses: string[],
): Promise<any> {
	const start = Date.now()
	while (Date.now() - start < timeoutMs) {
		const state = (await coreRelayMessageStatus(adapter, messageId)) as any
		if (state && statuses.includes(state.status)) return state
		await sleep(CORE_RELAY_POLL_INTERVAL_MS)
	}
	throw new RelayTimeoutError()
}

function sleep(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms))
}
