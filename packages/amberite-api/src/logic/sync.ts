import type { CoreApiClient } from '../client'
import type { CoreModLoader } from '../types'
import { getNextCoreServerPort, normalizeCoreLoader } from './core'

export interface SyncedLocalProfile {
	path: string
	name: string
	game_version: string
	loader: string
	loader_version?: string | null
	memory?: { maximum?: number | null } | null
	profile_type?: string
}

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

export interface ClientContentItemLike {
	file_name: string
	enabled?: boolean | null
	project?: { id?: string | null; title?: string | null } | null
	version?: { id?: string | null } | null
}

export interface ConvertToSyncedOptions<TProfile extends SyncedLocalProfile> {
	core: CoreApiClient
	profilePath: string
	getProfile: (path: string) => Promise<TProfile | null>
	editProfile: (path: string, edit: Partial<TProfile>) => Promise<unknown>
	setLinkedServerId: (profilePath: string, serverInstanceId: string) => void
	registerSyncedProfile?: (args: {
		profilePath: string
		serverInstanceId: string
		instance: Pick<TProfile, 'name' | 'game_version' | 'loader'>
	}) => Promise<unknown>
	memoryMb?: number
}

export async function convertProfileToSynced<TProfile extends SyncedLocalProfile>(
	options: ConvertToSyncedOptions<TProfile>,
): Promise<string> {
	const instance = await options.getProfile(options.profilePath)
	if (!instance) throw new Error(`Profile not found: ${options.profilePath}`)

	const existingServers = await options.core.listInstances()
	const port = getNextCoreServerPort(existingServers.map((server) => server.port))
	const coreInstance = await options.core.createInstance({
		name: instance.name,
		game_version: instance.game_version,
		loader: normalizeCoreLoader(instance.loader) as CoreModLoader,
		loader_version: instance.loader_version ?? undefined,
		port,
		memory: { min_mb: 1024, max_mb: options.memoryMb ?? instance.memory?.maximum ?? 4096 },
	})

	options.setLinkedServerId(options.profilePath, coreInstance.id)
	await options.editProfile(options.profilePath, { profile_type: 'synced' } as Partial<TProfile>)
	await options.registerSyncedProfile?.({
		profilePath: options.profilePath,
		serverInstanceId: coreInstance.id,
		instance,
	})
	return coreInstance.id
}

export async function buildSyncedManifest(options: {
	core: CoreApiClient
	instance: Pick<SyncedLocalProfile, 'path' | 'game_version' | 'loader'>
	serverInstanceId: string
	getClientContentItems: (profilePath: string) => Promise<ClientContentItemLike[]>
}): Promise<SyncedManifest> {
	const [clientItems, serverMods] = await Promise.all([
		options.getClientContentItems(options.instance.path).catch(() => []),
		options.core.listMods(options.serverInstanceId).catch(() => []),
	])

	return {
		generatedAt: Date.now(),
		gameVersion: options.instance.game_version,
		loader: options.instance.loader,
		client: clientItems.map((item) => ({
			projectId: item.project?.id ?? null,
			versionId: item.version?.id ?? null,
			name: item.project?.title ?? item.file_name,
			fileName: item.file_name,
			enabled: item.enabled ?? true,
		})),
		server: serverMods.map((mod) => ({
			projectId: mod.modrinth_project_id,
			versionId: mod.modrinth_version_id,
			name: mod.display_name ?? mod.filename,
			fileName: mod.filename,
			enabled: mod.enabled,
		})),
	}
}

export function canConvertProfileToSynced(
	instance: Pick<SyncedLocalProfile, 'profile_type'>,
): boolean {
	return instance.profile_type === 'client'
}
