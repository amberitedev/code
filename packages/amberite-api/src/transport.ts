import type { PlatformAdapter } from './adapter'
import { convexMutation, convexQuery } from './convex-relay'
import { publishCoreRelay } from './core-relay'
import { NetworkError, RelayTimeoutError } from './errors'

export type MessageMode = 'direct-queued' | 'direct-fire-and-forget' | 'core-relay' | 'convex-relay'
export type AckPolicy = 'none' | 'received' | 'processed'

export interface MessageDefinition<TPayload = unknown> {
	type: string
	version: number
	mode: MessageMode
	ack: AckPolicy
	ttlMs?: number
	payload?: TPayload
}

export interface MessageEnvelope<TPayload = unknown> {
	id: string
	type: string
	version: number
	mode: MessageMode
	ack: AckPolicy
	senderId: string
	recipientId: string
	payload: TPayload
	createdAt: number
	ttlMs: number
}

export interface PublishOptions<TPayload = unknown> {
	definition: MessageDefinition<TPayload>
	senderId: string
	recipientId: string
	payload: TPayload
	queueName?: string
}

const DEFAULT_TTL_MS = 5 * 60 * 1000
const RELAY_TIMEOUT_MS = 30_000

export const messageDefinitions = {
	coreHealthProbe: {
		type: 'core.health.probe',
		version: 1,
		mode: 'direct-fire-and-forget',
		ack: 'none',
	},
	coreConfigChanged: {
		type: 'core.config.changed',
		version: 1,
		mode: 'direct-queued',
		ack: 'received',
	},
	profileSync: {
		type: 'core.instance.profile-sync',
		version: 1,
		mode: 'core-relay',
		ack: 'processed',
	},
	coreHeartbeat: {
		type: 'core.registration.heartbeat',
		version: 1,
		mode: 'convex-relay',
		ack: 'received',
	},
	pairingCompleted: {
		type: 'core.pairing.completed',
		version: 1,
		mode: 'convex-relay',
		ack: 'processed',
	},
} satisfies Record<string, MessageDefinition>

export async function publishMessage<TPayload>(
	adapter: PlatformAdapter,
	options: PublishOptions<TPayload>,
): Promise<MessageEnvelope<TPayload>> {
	const envelope = createEnvelope(options)
	switch (envelope.mode) {
		case 'direct-queued':
			await adapter.queueStore?.push(options.queueName ?? 'direct', {
				id: envelope.id,
				createdAt: envelope.createdAt,
				payload: envelope,
			})
			return envelope
		case 'direct-fire-and-forget':
			return envelope
		case 'core-relay':
			await publishCoreRelay(adapter, envelope)
			return envelope
		case 'convex-relay':
			await convexMutation(adapter, 'messaging:publishMessage', {
				messageId: envelope.id,
				type: envelope.type,
				version: envelope.version,
				senderId: envelope.senderId,
				recipientId: envelope.recipientId,
				payload: envelope.payload,
				ack: envelope.ack,
				ttlMs: envelope.ttlMs,
			})
			return envelope
	}
}

export async function pendingMessages(
	adapter: PlatformAdapter,
	recipientId: string,
): Promise<unknown[]> {
	return await convexQuery(adapter, 'messaging:pendingMessages', { recipientId })
}

export async function writeReceipt(
	adapter: PlatformAdapter,
	messageId: string,
	recipientId: string,
): Promise<void> {
	await convexMutation(adapter, 'messaging:ackMessage', { messageId, recipientId })
}

export async function writeResult(
	adapter: PlatformAdapter,
	messageId: string,
	recipientId: string,
	result: unknown,
): Promise<void> {
	await convexMutation(adapter, 'messaging:completeMessage', { messageId, recipientId, result })
}

export async function waitForReceipt(
	adapter: PlatformAdapter,
	messageId: string,
	timeoutMs = RELAY_TIMEOUT_MS,
): Promise<void> {
	await waitForStatus(adapter, messageId, timeoutMs, ['received', 'processed'])
}

export async function waitForResult(
	adapter: PlatformAdapter,
	messageId: string,
	timeoutMs = RELAY_TIMEOUT_MS,
): Promise<unknown> {
	const status = await waitForStatus(adapter, messageId, timeoutMs, ['processed'])
	if (status?.error) throw new NetworkError(status.error)
	return status?.result
}

export async function heartbeatCore(
	adapter: PlatformAdapter,
	coreId: string,
	status?: string,
	metadata?: unknown,
): Promise<void> {
	await convexMutation(adapter, 'presence:heartbeatCore', { coreId, status, metadata })
}

export async function corePresence(adapter: PlatformAdapter, coreId: string): Promise<unknown> {
	return await convexQuery(adapter, 'presence:corePresence', { coreId })
}

export function createEnvelope<TPayload>(
	options: PublishOptions<TPayload>,
): MessageEnvelope<TPayload> {
	return {
		id: crypto.randomUUID(),
		type: options.definition.type,
		version: options.definition.version,
		mode: options.definition.mode,
		ack: options.definition.ack,
		senderId: options.senderId,
		recipientId: options.recipientId,
		payload: options.payload,
		createdAt: Date.now(),
		ttlMs: options.definition.ttlMs ?? DEFAULT_TTL_MS,
	}
}

async function waitForStatus(
	adapter: PlatformAdapter,
	messageId: string,
	timeoutMs: number,
	statuses: string[],
) {
	const start = Date.now()
	while (Date.now() - start < timeoutMs) {
		const state = await convexQuery<any>(adapter, 'messaging:messageStatus', { messageId })
		if (state && statuses.includes(state.status)) return state
		await sleep(500)
	}
	throw new RelayTimeoutError()
}

function sleep(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms))
}
