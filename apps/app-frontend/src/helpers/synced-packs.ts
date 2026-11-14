import { type PackSyncTarget, type SyncedPackType, syncedPacks } from '@modrinth/api-client'
import type { ContentItem } from '@modrinth/ui'
import { queryOptions } from '@tanstack/vue-query'

import { apiClient } from '@/platform/modrinth-client'

import { adaptContentItems, isSyncedOptionAvailable } from './instance'
import type { DesyncServerMode } from './worlds'

export type { SyncedPackType }
export type SyncedPackAction = 'enable' | 'disable'

export function isSyncedPackTypeAvailable(projectType: ContentItem['project_type']): boolean {
	return (
		['resourcepack', 'datapack'].includes(projectType) &&
		isSyncedOptionAvailable(projectType === 'resourcepack' ? 'resource_packs' : 'data_packs')
	)
}

export interface PackSyncPreview {
	pack: ContentItem
	instances: PackSyncTarget[]
}

export const syncedPackKeys = {
	all: ['synced-packs'] as const,
	list: (type: SyncedPackType) => ['synced-packs', 'list', type] as const,
}

export function syncedPackQueryOptions(type: SyncedPackType) {
	return queryOptions({
		queryKey: syncedPackKeys.list(type),
		networkMode: 'always',
		queryFn: () => list_synced_packs(type),
		enabled: isSyncedPackTypeAvailable(type),
	})
}

export async function list_synced_packs(projectType: SyncedPackType): Promise<ContentItem[]> {
	const items = (await apiClient.local.call(syncedPacks.listSyncedPacks, {
		query: { project_type: projectType },
	})) as ContentItem[]
	return adaptContentItems(items).map((item) => ({ ...item, external: !item.project }))
}

export async function get_pack_sync_preview(
	instanceId: string,
	projectPath: string,
): Promise<PackSyncPreview> {
	const preview = await apiClient.local.call(syncedPacks.previewPackSync, {
		params: { id: instanceId },
		query: { project_path: projectPath },
	})
	return { ...preview, pack: preview.pack as ContentItem }
}

export function sync_pack(instanceId: string, projectPath: string): Promise<void> {
	return apiClient.local.call(syncedPacks.syncPack, {
		params: { id: instanceId },
		body: { project_path: projectPath },
	})
}

export function desync_pack(
	instanceId: string,
	packId: string,
	mode: DesyncServerMode,
): Promise<void> {
	return apiClient.local.call(syncedPacks.desyncPack, {
		params: { id: instanceId, pack_id: packId },
		query: { mode },
	})
}

export function upload_synced_pack(
	path: string,
	projectType: SyncedPackType,
	gameVersions: string[],
): Promise<void> {
	return apiClient.local.call(syncedPacks.uploadSyncedPack, {
		body: { path, project_type: projectType, game_versions: gameVersions },
	})
}

export function set_synced_pack_enabled(packId: string, enabled: boolean): Promise<void> {
	return apiClient.local.call(syncedPacks.setSyncedPackEnabled, {
		params: { id: packId },
		body: { enabled },
	})
}

export function remove_synced_pack(packId: string): Promise<void> {
	return apiClient.local.call(syncedPacks.removeSyncedPack, { params: { id: packId } })
}
