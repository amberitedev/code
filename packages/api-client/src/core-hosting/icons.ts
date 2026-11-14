import { instanceFiles } from '@amberite/contracts'

import type { AbstractModrinthClient } from '../core/abstract-client'
import type { ArchonIconsV1Module } from '../modules/archon/icons/v1'
import { type Adapter, coreOf } from './adapter'

async function sha256(blob: Blob): Promise<string> {
	const digest = await crypto.subtle.digest('SHA-256', await blob.arrayBuffer())
	return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, '0')).join('')
}

/**
 * A server's icon is its instance's. The icon editor's settings stay with the instance, which a
 * Core does not serve whole (it names host paths), so a saved icon reads back as an image.
 */
export function icons(client: AbstractModrinthClient): Adapter<ArchonIconsV1Module> {
	const call = async (serverId: string) => {
		const api = await coreOf(client, serverId)
		return { api, params: { id: serverId } }
	}
	const get = async (serverId: string) => {
		const { api, params } = await call(serverId)
		return api.call(instanceFiles.GetInstanceIcon, { params })
	}
	const metadata = async (serverId: string) => ({
		hash: await sha256(await get(serverId)),
		config: null,
	})
	return {
		get: (serverId) => get(serverId),
		getMetadata: metadata,
		async set(serverId, image) {
			const { api, params } = await call(serverId)
			await api.call(instanceFiles.SetInstanceIcon, { params, body: image })
			return metadata(serverId)
		},
		async setGenerated(serverId, image, config) {
			const { api, params } = await call(serverId)
			const bytes = Array.from(new Uint8Array(await image.arrayBuffer()))
			await api.call(instanceFiles.SetEditedInstanceIcon, {
				params,
				body: { config, image: bytes },
			})
			return { ...(await metadata(serverId)), config }
		},
		async delete(serverId) {
			const { api, params } = await call(serverId)
			await api.call(instanceFiles.RemoveInstanceIcon, { params })
		},
	}
}
