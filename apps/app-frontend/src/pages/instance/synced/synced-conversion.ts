import type { CoreModLoader } from '@amberite/amberite-api'

import { useCoreClient } from '@/composables/useCoreClient'
import { edit, get } from '@/helpers/profile'
import type { GameInstance, InstanceLoader } from '@/helpers/types'

import { registerSyncedProfileBackend } from './synced-registration'
import { clearLinkedServerId, setLinkedServerId } from './use-synced-link'

/**
 * Synced profile conversion helpers.
 *
 * A "synced" profile is a single profile that surfaces both a client instance
 * (the profile itself) and a server instance (an Amberite Core instance). For
 * profiles created through the Synced creation flow the profile is named after
 * the Core instance id, so the two share an id. Converting an existing client
 * profile keeps its original path, so we provision a fresh Core instance and
 * record the `profilePath -> coreInstanceId` link (see use-synced-link.ts).
 */

function toCoreLoader(loader: InstanceLoader): CoreModLoader {
	if (['vanilla', 'fabric', 'forge', 'neoforge', 'quilt'].includes(loader)) {
		return loader as CoreModLoader
	}
	return 'vanilla'
}

function getNextServerPort(ports: number[]): number {
	const usedPorts = new Set(ports)
	let port = 25565
	while (usedPorts.has(port)) port += 1
	return port
}

/**
 * Flip an existing client profile into a synced profile, provisioning the Core
 * instance that backs its server side and linking the two.
 */
export async function convertToSynced(path: string): Promise<void> {
	const instance = await get(path)
	if (!instance) throw new Error(`Profile not found: ${path}`)

	const core = useCoreClient()
	const existingServers = await core.listInstances()
	const port = getNextServerPort(existingServers.map((server) => server.port))
	const coreInstance = await core.createInstance({
		name: instance.name,
		game_version: instance.game_version,
		loader: toCoreLoader(instance.loader),
		loader_version: instance.loader_version,
		port,
		memory: { min_mb: 1024, max_mb: instance.memory?.maximum ?? 4096 },
	})

	setLinkedServerId(path, coreInstance.id)
	await edit(path, { profile_type: 'synced' } as Partial<GameInstance>)
	await registerSyncedProfileBackend({
		profilePath: path,
		serverInstanceId: coreInstance.id,
		instance,
	})
}

/** Revert a synced profile back to a plain client profile. */
export async function convertToClient(path: string): Promise<void> {
	await edit(path, { profile_type: 'client' } as Partial<GameInstance>)
	clearLinkedServerId(path)
}

/** Whether a profile can be converted into a synced profile. */
export function canConvertToSynced(instance: Pick<GameInstance, 'profile_type'>): boolean {
	return instance.profile_type === 'client'
}
