/**
 * Shared transport layer for relay and push notifications via Supabase Realtime.
 *
 * Both Core and the desktop app maintain a persistent Realtime subscription.
 * When direct HTTP fails, messages are published to the relay table and picked
 * up by the other's subscription.
 *
 * Every message has two-phase acknowledgment:
 * 1. received_at — written immediately on pickup (confirms receiver is alive).
 * 2. completed_at + result — written when processing finishes.
 */

import type { SupabaseClient } from '@supabase/supabase-js'
import { NetworkError, RelayTimeoutError, CoreOfflineError } from './errors'

export type MessageDirection = 'client-to-core' | 'core-to-client'

export interface RelayMessage {
	id: string
	core_id: string
	sender_id: string
	direction: MessageDirection
	payload: unknown
	received_at: string | null
	completed_at: string | null
	result: unknown | null
	created_at: string
	ttl: string
}

export interface PublishOptions {
	coreId: string
	senderId: string
	direction: MessageDirection
	payload: unknown
	ttlSeconds?: number
}

const DEFAULT_TTL_SECONDS = 300
const RELAY_TIMEOUT_MS = 30_000

/**
 * Publish a message to the core_messages relay table.
 */
export async function publishMessage(
	supabase: SupabaseClient,
	opts: PublishOptions,
): Promise<RelayMessage> {
	const ttl = new Date(Date.now() + (opts.ttlSeconds ?? DEFAULT_TTL_SECONDS) * 1000).toISOString()
	const { data, error } = await supabase
		.from('core_messages')
		.insert({
			core_id: opts.coreId,
			sender_id: opts.senderId,
			direction: opts.direction,
			payload: opts.payload,
			ttl,
		})
		.select()
		.single()

	if (error) throw new NetworkError(`Failed to publish relay message: ${error.message}`)
	return data as RelayMessage
}

/**
 * Write received_at on a message to acknowledge receipt.
 */
export async function writeReceipt(supabase: SupabaseClient, messageId: string): Promise<void> {
	const { error } = await supabase
		.from('core_messages')
		.update({ received_at: new Date().toISOString() })
		.eq('id', messageId)

	if (error) throw new NetworkError(`Failed to write receipt: ${error.message}`)
}

/**
 * Write completed_at and result on a message when processing finishes.
 */
export async function writeResult(
	supabase: SupabaseClient,
	messageId: string,
	result: unknown,
): Promise<void> {
	const { error } = await supabase
		.from('core_messages')
		.update({ completed_at: new Date().toISOString(), result })
		.eq('id', messageId)

	if (error) throw new NetworkError(`Failed to write result: ${error.message}`)
}

/**
 * Wait for a message's received_at to be set by the receiver.
 * Throws RelayTimeoutError if not received within 30 seconds.
 */
export async function waitForReceipt(
	supabase: SupabaseClient,
	messageId: string,
	timeoutMs = RELAY_TIMEOUT_MS,
): Promise<void> {
	const start = Date.now()
	while (Date.now() - start < timeoutMs) {
		const { data, error } = await supabase
			.from('core_messages')
			.select('received_at')
			.eq('id', messageId)
			.single()

		if (error) {
			await sleep(500)
			continue
		}

		if (data?.received_at) return
		await sleep(500)
	}
	throw new RelayTimeoutError()
}

/**
 * Wait for a message's completed_at to be set by the receiver.
 */
export async function waitForResult(
	supabase: SupabaseClient,
	messageId: string,
	timeoutMs = RELAY_TIMEOUT_MS,
): Promise<unknown> {
	const start = Date.now()
	while (Date.now() - start < timeoutMs) {
		const { data, error } = await supabase
			.from('core_messages')
			.select('completed_at, result')
			.eq('id', messageId)
			.single()

		if (error) {
			await sleep(500)
			continue
		}

		if (data?.completed_at) return data.result
		await sleep(500)
	}
	throw new RelayTimeoutError()
}

/**
 * Subscribe to incoming messages for a given core_id and direction.
 * Calls onMessage for every new row. The caller is responsible for writing
 * receipt and result via writeReceipt / writeResult.
 */
export function subscribeToMessages(
	supabase: SupabaseClient,
	coreId: string,
	direction: MessageDirection,
	onMessage: (msg: RelayMessage) => void | Promise<void>,
): { unsubscribe: () => void } {
	const channel = supabase
		.channel(`core_messages:${coreId}:${direction}`)
		.on(
			'postgres_changes',
			{
				event: 'INSERT',
				schema: 'public',
				table: 'core_messages',
				filter: `core_id=eq.${coreId}`,
			},
			async (payload) => {
				const msg = payload.new as RelayMessage
				if (msg.direction !== direction) return
				await onMessage(msg)
			},
		)
		.subscribe()

	return {
		unsubscribe: () => {
			supabase.removeChannel(channel)
		},
	}
}

function sleep(ms: number): Promise<void> {
	return new Promise((res) => setTimeout(res, ms))
}
