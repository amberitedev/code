import { useCoreClient } from '@/composables/useCoreClient'
import { get_content_items } from '@/helpers/profile'
import type { GameInstance } from '@/helpers/types'

/**
 * Git-style content manifest for a synced profile.
 *
 * A snapshot captures the full content of both sides at a point in time — the
 * client profile's mods/resourcepacks and the Core server's tracked mods — so
 * the backend can diff successive snapshots and (eventually) apply the delta to
 * peers. Each entry is keyed by Modrinth project/version where known, falling
 * back to the on-disk file name.
 */
export interface SyncedManifestEntry {
	projectId: string | null
	versionId: string | null
	name: string
	fileName: string
	enabled: boolean
}

export interface SyncedManifest {
	generatedAt: number
	gameVersion: string
	loader: string
	client: SyncedManifestEntry[]
	server: SyncedManifestEntry[]
}

async function buildClientEntries(profilePath: string): Promise<SyncedManifestEntry[]> {
	const items = await get_content_items(profilePath).catch(() => [])
	return items.map((item) => ({
		projectId: item.project?.id ?? null,
		versionId: item.version?.id ?? null,
		name: item.project?.title ?? item.file_name,
		fileName: item.file_name,
		enabled: item.enabled ?? true,
	}))
}

async function buildServerEntries(serverInstanceId: string): Promise<SyncedManifestEntry[]> {
	const core = useCoreClient()
	const mods = await core.listMods(serverInstanceId).catch(() => [])
	return mods.map((mod) => ({
		projectId: mod.modrinth_project_id,
		versionId: mod.modrinth_version_id,
		name: mod.display_name ?? mod.filename,
		fileName: mod.filename,
		enabled: mod.enabled,
	}))
}

/** Read both sides' current content into a normalized snapshot manifest. */
export async function buildSyncedManifest(
	instance: Pick<GameInstance, 'path' | 'game_version' | 'loader'>,
	serverInstanceId: string,
): Promise<SyncedManifest> {
	const [client, server] = await Promise.all([
		buildClientEntries(instance.path),
		buildServerEntries(serverInstanceId),
	])
	return {
		generatedAt: Date.now(),
		gameVersion: instance.game_version,
		loader: instance.loader,
		client,
		server,
	}
}
