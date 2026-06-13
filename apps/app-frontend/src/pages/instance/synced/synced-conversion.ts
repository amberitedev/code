import { canConvertProfileToSynced, convertProfileToSynced } from '@amberite/amberite-api'

import { useCoreClient } from '@/composables/useCoreClient'
import { edit, get } from '@/helpers/profile'
import type { GameInstance } from '@/helpers/types'

import { registerSyncedProfileBackend } from './synced-registration'
import { clearLinkedServerId, setLinkedServerId } from './use-synced-link'

/**
 * Synced profile conversion helpers.
 *
 * A "synced" profile is a single profile that surfaces both a client instance
 * (the profile itself) and a server instance (an Copal instance). For
 * profiles created through the Synced creation flow the profile is named after
 * the Core instance id, so the two share an id. Converting an existing client
 * profile keeps its original path, so we provision a fresh Core instance and
 * record the `profilePath -> coreInstanceId` link (see use-synced-link.ts).
 */

/**
 * Flip an existing client profile into a synced profile, provisioning the Core
 * instance that backs its server side and linking the two.
 */
export async function convertToSynced(path: string): Promise<string> {
	return await convertProfileToSynced<GameInstance>({
		core: useCoreClient(),
		profilePath: path,
		getProfile: get,
		editProfile: edit,
		setLinkedServerId,
		registerSyncedProfile: registerSyncedProfileBackend,
	})
}

/** Revert a synced profile back to a plain client profile. */
export async function convertToClient(path: string): Promise<void> {
	await edit(path, { profile_type: 'client' } as Partial<GameInstance>)
	clearLinkedServerId(path)
}

/** Whether a profile can be converted into a synced profile. */
export function canConvertToSynced(instance: Pick<GameInstance, 'profile_type'>): boolean {
	return canConvertProfileToSynced(instance)
}
