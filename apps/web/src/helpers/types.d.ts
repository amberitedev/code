import type { ModrinthId } from '@modrinth/utils'

export type GameInstance = {
	path: string
	install_stage: InstallStage

	name: string
	icon_path?: string

	game_version: string
	loader: InstanceLoader
	loader_version?: string

	groups: string[]

	linked_data?: LinkedData

	created: Date
	modified: Date
	last_played?: Date

	submitted_time_played: number
	recent_time_played: number

	java_path?: string
	extra_launch_args?: string[]
	custom_env_vars?: [string, string][]

	memory?: MemorySettings
	force_fullscreen?: boolean
	game_resolution?: [number, number]
	hooks: Hooks
}

type InstallStage =
	| 'installed'
	| 'minecraft_installing'
	| 'pack_installed'
	| 'pack_installing'
	| 'not_installed'

type LinkedData = {
	project_id: ModrinthId
	version_id: ModrinthId

	locked: boolean
}

export type InstanceLoader = 'vanilla' | 'forge' | 'fabric' | 'quilt' | 'neoforge'

type ContentFile = {
	metadata?: {
		project_id: string
		version_id: string
	}
}

type ContentFileProjectType = 'mod' | 'datapack' | 'resourcepack' | 'shaderpack'

type CacheBehaviour =
	| 'stale_while_revalidate_skip_offline'
	| 'stale_while_revalidate'
	| 'must_revalidate'
	| 'bypass'

type MemorySettings = {
	maximum: number
}

type WindowSize = {
	width: number
	height: number
}

type Hooks = {
	pre_launch?: string
	wrapper?: string
	post_exit?: string
}

type Manifest = {
	gameVersions: ManifestGameVersion[]
}

type ManifestGameVersion = {
	id: string
	stable: boolean
	loaders: ManifestLoaderVersion[]
}

type ManifestLoaderVersion = {
	id: string
	url: string
	stable: boolean
}
