/**
 * Mocked head-storage helper - uses IndexedDB (browser-native)
 * Original: used Tauri file system, now uses browser IndexedDB
 */

interface StoredHead {
	blob: Blob
	timestamp: number
}

export class HeadStorage {
	private dbName = 'head-storage'
	private version = 1
	private db: IDBDatabase | null = null

	async init(): Promise<void> {
		return new Promise((resolve, reject) => {
			const request = indexedDB.open(this.dbName, this.version)

			request.onerror = () => reject(request.error)
			request.onsuccess = () => {
				this.db = request.result
				resolve()
			}

			request.onupgradeneeded = () => {
				const db = request.result
				if (!db.objectStoreNames.contains('heads')) {
					db.createObjectStore('heads')
				}
			}
		})
	}

	async store(_key: string, _blob: Blob): Promise<void> {
		// no-op in web mode - heads are generated on-demand
	}

	async retrieve(_key: string): Promise<string | null> {
		return null
	}

	async batchRetrieve(_keys: string[]): Promise<Record<string, Blob | null>> {
		const results: Record<string, Blob | null> = {}
		for (const key of _keys) {
			results[key] = null
		}
		return results
	}

	async cleanupInvalidKeys(_validKeys: Set<string>): Promise<number> {
		return 0
	}

	async debugCalculateStorage(): Promise<void> {
		// no-op
	}

	async clearAll(): Promise<void> {
		// no-op
	}
}

export const headStorage = new HeadStorage()
