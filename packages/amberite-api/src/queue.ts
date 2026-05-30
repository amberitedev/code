import type { PersistentQueueStore, QueuedMessage } from './adapter'

export class MemoryQueueStore implements PersistentQueueStore {
	private queues = new Map<string, QueuedMessage[]>()

	async list(queueName: string): Promise<QueuedMessage[]> {
		return [...(this.queues.get(queueName) ?? [])]
	}

	async push(queueName: string, message: QueuedMessage): Promise<void> {
		const queue = this.queues.get(queueName) ?? []
		this.queues.set(queueName, [...queue, message])
	}

	async remove(queueName: string, id: string): Promise<void> {
		const queue = this.queues.get(queueName) ?? []
		this.queues.set(
			queueName,
			queue.filter((message) => message.id !== id),
		)
	}
}

export class CompositeQueueStore implements PersistentQueueStore {
	constructor(private readonly stores: PersistentQueueStore[]) {}

	async list(queueName: string): Promise<QueuedMessage[]> {
		const queues = await Promise.all(this.stores.map((store) => store.list(queueName)))
		return queues.flat()
	}

	async push(queueName: string, message: QueuedMessage): Promise<void> {
		await Promise.all(this.stores.map((store) => store.push(queueName, message)))
	}

	async remove(queueName: string, id: string): Promise<void> {
		await Promise.all(this.stores.map((store) => store.remove(queueName, id)))
	}
}
