import type { PlatformAdapter } from './adapter'
import { publishCoreRelay } from './core-relay'
import type { MessageEnvelope } from './transport'

/**
 * Drain the local direct-queued message queue, delivering each stored envelope
 * to Core via the relay endpoint. Call this when Core transitions to online-direct.
 *
 * Each message is removed from the queue only after successful delivery.
 * Failed deliveries remain in the queue for the next drain cycle.
 */
export async function drainQueue(adapter: PlatformAdapter): Promise<void> {
	if (!adapter.queueStore) return
	const messages = await adapter.queueStore.list('direct')
	for (const queued of messages) {
		try {
			await publishCoreRelay(adapter, queued.payload as MessageEnvelope)
			await adapter.queueStore.remove('direct', queued.id)
		} catch {
			// Leave message in queue; will retry on next drain cycle.
		}
	}
}
