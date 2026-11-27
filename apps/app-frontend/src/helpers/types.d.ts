import type {
	GameInstance as InstanceResponse,
	InstanceLink as InstanceLinkResponse,
	InstanceSyncedOptions,
} from '@amberite/contracts/engine'
import type { ModrinthId } from '@modrinth/utils'

export type { Settings as AppSettings } from '@amberite/contracts/app'
export type { ModLoader as InstanceLoader } from '@amberite/contracts/protocol'
export type {
	CacheBehaviour,
	ContentFile,
	SharedInstanceAttachment,
} from '@amberite/contracts/engine'

/** The engine's instance with the options it shares and its groups, which the App keeps. */
export type GameInstance = Omit<InstanceResponse, 'link'> & {
	link: InstanceLink | null
	synced_options: InstanceSyncedOptions
	group_ids: string[]
}

/** Every variant's ids, readable without narrowing. */
type InstanceLinkIdentity = {
	project_id?: ModrinthId | null
	version_id?: ModrinthId | null
	server_project_id?: ModrinthId | null
	content_project_id?: ModrinthId | null
	content_version_id?: ModrinthId | null
}

type InstanceLink = InstanceLinkIdentity & InstanceLinkResponse

export type ContentFileProjectType = 'mod' | 'datapack' | 'resourcepack' | 'shaderpack'

export type Manifest = {
	gameVersions: ManifestGameVersion[]
	versionGroups?: ManifestVersionGroup[]
}

type ManifestGameVersion = {
	id: string
	stable: boolean
	versionGroup?: string
	loaders: ManifestLoaderVersion[]
}

type ManifestVersionGroup = {
	id: string
	loaders: ManifestLoaderVersion[]
}

type ManifestLoaderVersion = {
	id: string
	url: string
	stable: boolean
}
