import type { CoreApiClient } from '../client'
import type { CoreModLoader, CoreSyncProfile } from '../types'
import { getNextCoreServerPort, normalizeCoreLoader } from './core'

const CLIENT_SYNC_LOADERS = new Set(['vanilla', 'fabric', 'forge', 'quilt', 'neoforge'])

export interface SyncedLocalProfile {
	path: string
	name: string
	game_version: string
	loader: string
	loader_version?: string | null
	memory?: { maximum?: number | null } | null
	profile_type?: string
	core_instance_id?: string | null
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

export interface FriendGroupSyncedProfileLike {
	_id?: string
	id?: string
	coreInstanceId?: string | null
	clientProfileId?: string | null
	name: string
	gameVersion?: string | null
	loader?: string | null
	syncEnabled?: boolean | null
	status?: string | null
}

export interface LocalSyncedProfileLike {
	path: string
	profile_type?: string | null
	core_instance_id?: string | null
}

export type InstallableSyncedProfileAvailability =
	| 'installable'
	| 'core-unavailable'
	| 'missing-core-profile'
	| 'missing-snapshot'
	| 'missing-metadata'

export interface InstallableSyncedProfile {
	socialProfileId: string
	coreProfileId: string | null
	coreInstanceId: string
	clientProfileId: string | null
	name: string
	gameVersion: string | null
	loader: string | null
	currentSnapshotId: string | null
	availability: InstallableSyncedProfileAvailability
	unavailableReason: string | null
}

export interface ResolveInstallableSyncedProfilesOptions {
	socialProfiles: FriendGroupSyncedProfileLike[]
	coreProfiles?: CoreSyncProfile[] | null
	localProfiles: LocalSyncedProfileLike[]
	coreAvailable?: boolean
}

export interface InstallSyncedProfileFromCoreOptions<TProfile> {
	core: Pick<CoreApiClient, 'downloadSyncSnapshot'>
	profile: InstallableSyncedProfile
	createProfile: (args: {
		name: string
		gameVersion: string
		loader: string
		profileType: 'synced'
	}) => Promise<string>
	getProfileFullPath: (profilePath: string) => Promise<string>
	joinPath: (...paths: string[]) => Promise<string>
	writeFile: (path: string, data: Uint8Array) => Promise<unknown>
	removeFile?: (path: string) => Promise<unknown>
	installMrpackFromPath: (mrpackPath: string, profilePath: string) => Promise<unknown>
	editProfile: (profilePath: string, edit: Partial<TProfile>) => Promise<unknown>
	removeProfile?: (profilePath: string) => Promise<unknown>
	linkServerId?: (profilePath: string, coreInstanceId: string) => void
	linkServerPath?: (profilePath: string, coreInstancePath: string) => void
}

export function normalizeClientSyncLoader(loader?: string | null): string | null {
	if (!loader) return null
	const normalized = loader.toLowerCase().replace(/[_-]/g, '')
	if (normalized === 'neoforge') return 'neoforge'
	return CLIENT_SYNC_LOADERS.has(normalized) ? normalized : null
}

export function resolveInstallableSyncedProfiles(
	options: ResolveInstallableSyncedProfilesOptions,
): InstallableSyncedProfile[] {
	const installedCoreInstanceIds = new Set(
		options.localProfiles
			.filter((profile) => profile.profile_type === 'synced' && profile.core_instance_id)
			.map((profile) => profile.core_instance_id as string),
	)
	const coreProfilesByInstanceId = new Map(
		(options.coreProfiles ?? [])
			.filter((profile) => profile.core_instance_id)
			.map((profile) => [profile.core_instance_id as string, profile]),
	)
	const coreAvailable = options.coreAvailable ?? options.coreProfiles != null

	return options.socialProfiles.flatMap((socialProfile) => {
		if (socialProfile.syncEnabled === false) return []
		if (socialProfile.status === 'archived') return []
		if (!socialProfile.coreInstanceId) return []
		if (installedCoreInstanceIds.has(socialProfile.coreInstanceId)) return []

		const coreProfile = coreProfilesByInstanceId.get(socialProfile.coreInstanceId) ?? null
		const loader = normalizeClientSyncLoader(coreProfile?.loader ?? socialProfile.loader)
		if (!loader) return []

		const gameVersion = coreProfile?.game_version ?? socialProfile.gameVersion ?? null
		let availability: InstallableSyncedProfileAvailability = 'installable'
		let unavailableReason: string | null = null

		if (!gameVersion) {
			availability = 'missing-metadata'
			unavailableReason = 'This shared instance is missing its Minecraft version.'
		} else if (!coreAvailable) {
			availability = 'core-unavailable'
			unavailableReason = 'Core must be online to install this shared instance.'
		} else if (!coreProfile) {
			availability = 'missing-core-profile'
			unavailableReason = 'Core does not have the synced profile for this instance.'
		} else if (!coreProfile.current_snapshot_id) {
			availability = 'missing-snapshot'
			unavailableReason = 'This shared instance has not published an installable version yet.'
		}

		return [
			{
				socialProfileId: socialProfile._id ?? socialProfile.id ?? socialProfile.coreInstanceId,
				coreProfileId: coreProfile?.id ?? null,
				coreInstanceId: socialProfile.coreInstanceId,
				clientProfileId: coreProfile?.client_profile_id ?? socialProfile.clientProfileId ?? null,
				name: coreProfile?.name ?? socialProfile.name,
				gameVersion,
				loader,
				currentSnapshotId: coreProfile?.current_snapshot_id ?? null,
				availability,
				unavailableReason,
			},
		]
	})
}

export async function installSyncedProfileFromCore<TProfile>(
	options: InstallSyncedProfileFromCoreOptions<TProfile>,
): Promise<string> {
	const { profile } = options
	const loader = normalizeClientSyncLoader(profile.loader)
	if (
		profile.availability !== 'installable' ||
		!profile.coreProfileId ||
		!profile.currentSnapshotId ||
		!profile.gameVersion ||
		!loader
	) {
		throw new Error(profile.unavailableReason ?? 'This shared instance is not installable.')
	}

	let createdProfilePath: string | null = null
	let archivePath: string | null = null

	try {
		const archive = await options.core.downloadSyncSnapshot(
			profile.coreProfileId,
			profile.currentSnapshotId,
		)
		const archiveBytes = new Uint8Array(await archive.arrayBuffer())
		createdProfilePath = await options.createProfile({
			name: profile.name,
			gameVersion: profile.gameVersion,
			loader,
			profileType: 'synced',
		})
		const profileFullPath = await options.getProfileFullPath(createdProfilePath)
		archivePath = await options.joinPath(
			profileFullPath,
			`${safeFilePart(profile.coreProfileId)}-${safeFilePart(profile.currentSnapshotId)}.mrpack`,
		)

		await options.writeFile(archivePath, archiveBytes)
		await options.installMrpackFromPath(archivePath, createdProfilePath)
		await options.editProfile(createdProfilePath, {
			profile_type: 'synced',
			core_instance_id: profile.coreInstanceId,
		} as Partial<TProfile>)
		options.linkServerId?.(createdProfilePath, profile.coreInstanceId)
		return createdProfilePath
	} catch (error) {
		if (createdProfilePath) {
			await options.removeProfile?.(createdProfilePath).catch(() => undefined)
		}
		throw error
	} finally {
		if (archivePath) {
			await options.removeFile?.(archivePath).catch(() => undefined)
		}
	}
}

function safeFilePart(value: string): string {
	return value.replace(/[^a-z0-9._-]/gi, '-')
}

export interface ConvertToSyncedOptions<TProfile extends SyncedLocalProfile> {
	core: CoreApiClient
	profilePath: string
	getProfile: (path: string) => Promise<TProfile | null>
	editProfile: (path: string, edit: Partial<TProfile>) => Promise<unknown>
	setLinkedServerId: (profilePath: string, serverInstanceId: string) => void
	setLinkedServerPath?: (profilePath: string, serverInstancePath: string) => void
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
	options.setLinkedServerPath?.(options.profilePath, coreInstance.path)
	await options.editProfile(options.profilePath, {
		profile_type: 'synced',
		core_instance_id: coreInstance.id,
	} as Partial<TProfile>)
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
