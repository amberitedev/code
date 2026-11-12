import type {
	GameInstance as InstanceResponse,
	InstanceLink as InstanceLinkResponse,
} from '@modrinth/api-client'
import type { ModrinthId } from '@modrinth/utils'

export type { SharedInstanceAttachment } from '@modrinth/api-client'

export type GameInstance = Omit<InstanceResponse, 'link'> & { link: InstanceLink | null }

/** Every variant's ids, readable without narrowing. */
type InstanceLinkIdentity = {
	project_id?: ModrinthId | null
	version_id?: ModrinthId | null
	server_project_id?: ModrinthId | null
	content_project_id?: ModrinthId | null
	content_version_id?: ModrinthId | null
}

export type InstanceLink = InstanceLinkIdentity & InstanceLinkResponse

export type Instance = GameInstance

export type InstanceLoader = 'vanilla' | 'forge' | 'fabric' | 'quilt' | 'neoforge'

export type ContentSourceKind =
	| 'local'
	| 'modrinth_modpack'
	| 'server_project'
	| 'modrinth_hosting'
	| 'imported_modpack'
	| 'shared_instance'

export type ContentFile = {
	enabled: boolean
	locked: boolean
	source_kind?: ContentSourceKind | null
	metadata?: {
		project_id: string
		version_id: string
	}
}

export type ContentFileProjectType = 'mod' | 'datapack' | 'resourcepack' | 'shaderpack'

export type CacheBehaviour =
	// Serve expired data. If fetch fails / launcher is offline, errors are ignored
	| 'stale_while_revalidate_skip_offline'
	// Serve expired data, revalidate in background
	| 'stale_while_revalidate'
	// Must revalidate if data is expired
	| 'must_revalidate'
	// Ignore cache- always fetch updated data from origin
	| 'bypass'

export type MemorySettings = {
	maximum: number
}

export type WindowSize = [number, number]

export type Hooks = {
	pre_launch?: string
	wrapper?: string
	post_exit?: string
}

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

export type AppSettings = {
	max_concurrent_downloads: number
	max_concurrent_writes: number

	theme: 'dark' | 'light' | 'oled' | 'retro' | 'system'
	default_page: 'Home' | 'Library'
	collapsed_navigation: boolean
	advanced_rendering: boolean
	native_decorations: boolean
	worlds_in_home: boolean
	sync_theme_across_devices: boolean
	sync_behavior_across_devices: boolean
	sync_features_across_devices: boolean
	show_files_tab_in_instances: boolean
	show_worlds_tab_in_instances: boolean
	show_screenshots_tab_in_instances: boolean
	show_skin_selector_in_sidebar: boolean

	telemetry: boolean
	discord_rpc: boolean
	developer_mode: boolean
	personalized_ads: boolean

	extra_launch_args: string[]
	custom_env_vars: [string, string][]
	memory: MemorySettings
	force_fullscreen: boolean
	game_resolution: [number, number]
	hide_on_process_start: boolean
	show_jump_in: boolean
	always_show_copy_details: boolean
	hide_installed_modpacks: boolean
	advanced_filters_collapsed: boolean
	dismissed_photosensitivity_filter_warning: boolean
	friends_active_collapsed: boolean
	friends_online_collapsed: boolean
	friends_offline_collapsed: boolean
	friends_pending_collapsed: boolean
	refocus_on_game_close: boolean
	compact_instance_cards: boolean
	show_play_time: boolean
	warn_on_unknown_modpacks: boolean
	skip_non_essential_warnings: boolean
	hooks: Hooks

	custom_dir?: string
	prev_custom_dir?: string
	migrated: boolean
}
