import { buildSyncedManifest as buildSyncedManifestWithAdapters } from '@amberite/amberite-api'

import { useCoreClient } from '@/composables/useCoreClient'
import { get_content_items } from '@/helpers/profile'
import type { GameInstance } from '@/helpers/types'
import type { SyncedManifest, SyncedManifestEntry } from '@amberite/amberite-api'

export type { SyncedManifest, SyncedManifestEntry }

/** Read both sides' current content into a normalized snapshot manifest. */
export async function buildSyncedManifest(
	instance: Pick<GameInstance, 'path' | 'game_version' | 'loader'>,
	serverInstanceId: string,
): Promise<SyncedManifest> {
	return await buildSyncedManifestWithAdapters({
		core: useCoreClient(),
		instance,
		serverInstanceId,
		getClientContentItems: get_content_items,
	})
}
