/**
 * Mocked skin-preview-storage helper - uses IndexedDB (browser-native)
 * Original: used Tauri file system, now uses browser IndexedDB
 */

import type { RawRenderResult } from '../rendering/batch-skin-renderer'

interface StoredPreview {
	forwards: Blob
	backwards: Blob
	timestamp: number
}

export class SkinPreviewStorage {
	private dbName = 'skin-previews'
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
				if (!db.objectStoreNames.contains('previews')) {
					db.createObjectStore('previews')
				}
			}
		})
	}

	async store(_key: string, _result: RawRenderResult): Promise<void> {
		// no-op in web mode - previews are generated on-demand
	}

	async retrieve(_key: string): Promise<RawRenderResult | null> {
		return null
	}

	async batchRetrieve(_keys: string[]): Promise<Record<string, RawRenderResult | null>> {
		const results: Record<string, RawRenderResult | null> = {}
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
}

export const skinPreviewStorage = new SkinPreviewStorage()
