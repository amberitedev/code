import type { ModrinthId } from '@modrinth/utils'

export type GameInstance = {
	id: string
	path: string
	install_stage: InstallStage
	profile_type: ProfileType
	core_instance_id?: string | null
	server_manifest_json?: ServerManifest | null

	name: string
	icon_path?: string

	game_version: string
	protocol_version?: number
	loader: InstanceLoader
	loader_version?: string

	groups: string[]

	link?: InstanceLink | null
	shared_instance?: SharedInstanceAttachment | null
	quarantined: boolean
	update_channel: ReleaseChannel

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

export type ProfileType = 'client' | 'server' | 'synced'

type LinkedData = {
	project_id: ModrinthId
	version_id: ModrinthId

	locked: boolean
}

export type ServerManifest = {
	name: string
	gameVersion: string
	modloader: string
	loaderVersion?: string | null
	port?: number
	memory?: {
		min_mb: number
		max_mb: number
	}
	content?: unknown
	mods?: unknown[]
}

type ReleaseChannel = 'release' | 'beta' | 'alpha'

export type InstanceLoader = 'vanilla' | 'forge' | 'fabric' | 'quilt' | 'neoforge'

export type ContentSourceKind =
	| 'local'
	| 'modrinth_modpack'
	| 'server_project'
	| 'modrinth_hosting'
	| 'imported_modpack'
	| 'shared_instance'

type ContentFile = {
	enabled: boolean
	source_kind?: ContentSourceKind | null
	metadata?: {
		project_id: string
		version_id: string
	}
}

type ContentFileProjectType = 'mod' | 'datapack' | 'resourcepack' | 'shaderpack'

type CacheBehaviour =
	// Serve expired data. If fetch fails / launcher is offline, errors are ignored
	| 'stale_while_revalidate_skip_offline'
	// Serve expired data, revalidate in background
	| 'stale_while_revalidate'
	// Must revalidate if data is expired
	| 'must_revalidate'
	// Ignore cache- always fetch updated data from origin
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

type AppSettings = {
	max_concurrent_downloads: number
	max_concurrent_writes: number

	theme: 'dark' | 'light' | 'oled'
	default_page: 'Home' | 'Library'
	collapsed_navigation: boolean
	advanced_rendering: boolean
	native_decorations: boolean
	worlds_in_home: boolean

	telemetry: boolean
	discord_rpc: boolean
	developer_mode: boolean
	personalized_ads: boolean

	onboarded: boolean

	extra_launch_args: string[]
	custom_env_vars: [string, string][]
	memory: MemorySettings
	force_fullscreen: boolean
	game_resolution: [number, number]
	hide_on_process_start: boolean
	hooks: Hooks

	custom_dir?: string
	prev_custom_dir?: string
	migrated: boolean
}
