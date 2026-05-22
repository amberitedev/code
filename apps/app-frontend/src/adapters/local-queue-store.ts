import type { PersistentQueueStore, QueuedMessage } from '@amberite/amberite-api'

const STORAGE_KEY_PREFIX = 'amberite:queue:'

/**
 * localStorage-backed PersistentQueueStore for the desktop app.
 *
 * Tauri's WebView localStorage survives app restarts, making it suitable for
 * durable message queuing. Entries are stored as a JSON array per queue name.
 */
export function createLocalQueueStore(): PersistentQueueStore {
	function storageKey(queueName: string): string {
		return `${STORAGE_KEY_PREFIX}${queueName}`
	}

	function readAll(queueName: string): QueuedMessage[] {
		try {
			const raw = localStorage.getItem(storageKey(queueName))
			if (!raw) return []
			return JSON.parse(raw) as QueuedMessage[]
		} catch {
			return []
		}
	}

	function writeAll(queueName: string, messages: QueuedMessage[]): void {
		localStorage.setItem(storageKey(queueName), JSON.stringify(messages))
	}

	return {
		async list(queueName) {
			return readAll(queueName)
		},

		async push(queueName, message) {
			const messages = readAll(queueName)
			messages.push(message)
			writeAll(queueName, messages)
		},

		async remove(queueName, id) {
			const messages = readAll(queueName).filter((m) => m.id !== id)
			writeAll(queueName, messages)
		},
	}
}
