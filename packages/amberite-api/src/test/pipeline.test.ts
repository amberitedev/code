import { describe, expect, it, vi } from 'vitest'
import type { PlatformAdapter } from '../adapter'
import { NetworkError } from '../errors'
import { CommunicationPipeline } from '../pipeline'
import { MemoryQueueStore } from '../queue'

function adapterWithQueue(queueStore = new MemoryQueueStore()): PlatformAdapter {
	return {
		fetchFn: vi.fn() as unknown as typeof fetch,
		convexUrl: '',
		queueStore,
		getCoreUrl: async () => 'http://localhost:16662',
		getCurrentJwt: async () => null,
		openExternalAuth: vi.fn(),
	}
}

describe('CommunicationPipeline', () => {
	it('retries transient network errors', async () => {
		const pipeline = new CommunicationPipeline(adapterWithQueue())
		const execute = vi
			.fn()
			.mockRejectedValueOnce(new NetworkError('temporary'))
			.mockResolvedValueOnce('ok')

		const result = await pipeline.call({
			key: 'test.retry',
			policy: { retries: 1, retryDelayMs: 0 },
			execute,
		})

		expect(result.ok).toBe(true)
		expect(result.value).toBe('ok')
		expect(result.attempts).toBe(2)
		expect(execute).toHaveBeenCalledTimes(2)
	})

	it('falls back to persistent queue for payload-bearing calls', async () => {
		const queueStore = new MemoryQueueStore()
		const pipeline = new CommunicationPipeline(adapterWithQueue(queueStore))

		const result = await pipeline.call({
			key: 'test.queue-call',
			payload: { thing: true },
			policy: {
				methods: ['core-direct', 'persistent-queue'],
				throwOnError: false,
			},
			execute: async () => {
				throw new NetworkError('offline')
			},
		})

		const queued = await queueStore.list('direct')
		expect(result.ok).toBe(true)
		expect(result.queued).toBe(true)
		expect(queued).toHaveLength(1)
		expect((queued[0]?.payload as any).payload).toEqual({ thing: true })
	})

	it('publishes queued messages through the configured queue', async () => {
		const queueStore = new MemoryQueueStore()
		const pipeline = new CommunicationPipeline(adapterWithQueue(queueStore))

		const result = await pipeline.publish({
			key: 'test.publish-queue',
			definition: {
				type: 'test.message',
				version: 1,
				mode: 'direct-fire-and-forget',
				ack: 'none',
			},
			senderId: 'sender',
			recipientId: 'recipient',
			payload: { hello: 'world' },
			policy: {
				methods: ['persistent-queue'],
				queueName: 'messages',
			},
		})

		const queued = await queueStore.list('messages')
		expect(result.ok).toBe(true)
		expect(result.queued).toBe(true)
		expect(queued).toHaveLength(1)
		expect((queued[0]?.payload as any).payload).toEqual({ hello: 'world' })
	})
})
