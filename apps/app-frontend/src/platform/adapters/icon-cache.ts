import { ContractClient, localFiles } from '@modrinth/api-client'
import type { IconCacheContext } from '@modrinth/ui'

import { cacheIconBytes } from '@/platform/app-lib/icons/commands'
import { localFileUrl, localTransport } from '@/platform/local'

export function createIconCache(): IconCacheContext {
	const local = new ContractClient(localTransport)
	const pending = new Map<string, Promise<string>>()
	const blobs = new WeakMap<Blob, Promise<string>>()
	const resolvedUrls = new Map<string, string>()
	const resolvedBlobs = new WeakMap<Blob, string>()
	return {
		cacheIcon(source) {
			if (typeof source !== 'string') {
				const existing = blobs.get(source)
				if (existing) return existing
				const request = source
					.arrayBuffer()
					.then((buffer) => cacheIconBytes(Array.from(new Uint8Array(buffer))))
					.then(localFileUrl)
					.then((url) => {
						resolvedBlobs.set(source, url)
						return url
					})
					.catch((error) => {
						blobs.delete(source)
						throw error
					})
				blobs.set(source, request)
				return request
			}
			if (!source.startsWith('https://')) return Promise.resolve(source)
			const existing = pending.get(source)
			if (existing) return existing
			const request = local
				.call(localFiles.cacheRemoteIcon, { body: { source } })
				.then(localFileUrl)
				.then((url) => {
					resolvedUrls.set(source, url)
					return url
				})
				.finally(() => {
					pending.delete(source)
				})
			pending.set(source, request)
			return request
		},
		peekIcon(source) {
			return typeof source === 'string' ? resolvedUrls.get(source) : resolvedBlobs.get(source)
		},
	}
}
