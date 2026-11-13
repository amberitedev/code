// Generated from packages/engine by ts-rs. Do not edit.
// Regenerate: cargo test -p engine -- --ignored write_ts

import type { Endpoint, SharedIconConfig } from './contracts'

export type EngineEvent = { "type": "loading", "payload": LoadingPayload } | { "type": "process", "payload": ProcessPayload } | { "type": "instance", "payload": InstancePayload } | { "type": "install_job", "payload": InstallJobSnapshot } | { "type": "warning", "payload": WarningPayload } | { "type": "log", "payload": LogPayload } | { "type": "ads_consent_required", "payload": boolean };

export type LoadingPayload = { event: LoadingBarType, loader_uuid: string, fraction: number | null, message: string, };

export type LoadingBarType = { "type": "legacy_data_migration" } | { "type": "directory_move", old: string, new: string, } | { "type": "java_download", version: number, } | { "type": "pack_file_download", instance_id: string, pack_name: string, icon: string | null, pack_version: string, } | { "type": "pack_download", instance_id: string, pack_name: string, icon: string | null, pack_id: string | null, pack_version: string | null, } | { "type": "pack_import", pack_name: string, } | { "type": "minecraft_download", instance_id: string, instance_name: string, } | { "type": "zip_extract", instance_id: string, instance_name: string, } | { "type": "pack_export", instance_id: string, instance_name: string, } | { "type": "config_change", new_path: string, } | { "type": "copy_instance", import_location: string, instance_name: string, } | { "type": "launcher_update", version: string, current_version: string, };

export type ProcessPayload = { instance_id: string, uuid: string, event: ProcessPayloadType, message: string, };

export type ProcessPayloadType = "launched" | "finished";

export type InstancePayload = { instance_id: string, } & ({ "event": "created" } | { "event": "synced" } | { "event": "servers_updated" } | { "event": "screenshots_updated" } | { "event": "world_updated", world: string, } | { "event": "server_joined", host: string, port: number, timestamp: string, } | { "event": "edited" } | { "event": "content_install_finished", project_ids: Array<string>, } | { "event": "content_install_failed", project_ids: Array<string>, message: string, } | { "event": "removed" });

export type InstancePayloadType = { "event": "created" } | { "event": "synced" } | { "event": "servers_updated" } | { "event": "screenshots_updated" } | { "event": "world_updated", world: string, } | { "event": "server_joined", host: string, port: number, timestamp: string, } | { "event": "edited" } | { "event": "content_install_finished", project_ids: Array<string>, } | { "event": "content_install_failed", project_ids: Array<string>, message: string, } | { "event": "removed" };

export type InstallJobSnapshot = { content_count: number | null, job_id: string, instance_id: string | null, kind: InstallJobKind, status: InstallJobStatus, paused: boolean, canceling: boolean, can_pause: boolean, can_cancel: boolean, target: InstallTarget, phase: InstallPhaseId, progress: InstallProgress | null, details: InstallPhaseDetails, display: InstallJobDisplay | null, error: InstallErrorView | null, rollback_error: InstallErrorView | null, created: string, modified: string, finished: string | null, };

export type InstallJobKind = "create_instance" | "create_modpack_instance" | "create_shared_instance" | "import_instance" | "duplicate_instance" | "install_existing_instance" | "install_pack_to_existing_instance" | "update_shared_instance" | "bulk_update_content";

export type InstallJobStatus = "queued" | "running" | "succeeded" | "failed" | "interrupted" | "canceled";

export type InstallTarget = { "type": "new_instance", instance_id: string | null, } | { "type": "existing_instance", instance_id: string, };

export type InstallPhaseId = "preparing_instance" | "resolving_pack" | "downloading_pack_file" | "reading_pack_manifest" | "downloading_content" | "extracting_overrides" | "resolving_minecraft" | "resolving_loader" | "preparing_java" | "downloading_minecraft" | "running_loader_processors" | "finalizing" | "rolling_back";

export type InstallProgress = { current: number, total: number, secondary?: InstallProgressSecondary, };

export type InstallProgressSecondary = { current: number, total: number, };

export type InstallPhaseDetails = { "type": "empty" } | { "type": "instance", name: string, } | { "type": "minecraft", game_version: string, loader: ModLoader, } | { "type": "java", major_version: number, step: InstallJavaStep, } | { "type": "modpack", project_id: string | null, version_id: string | null, title: string | null, } | { "type": "import", launcher_type: ImportLauncherType, instance_folder: string, };

export type InstallJavaStep = "resolving" | "fetching_metadata" | "downloading" | "extracting" | "validating";

export type InstallJobDisplay = { title: string, icon: string | null, };

export type InstallErrorView = { code: string, phase?: InstallPhaseId, message: string, api?: InstallApiErrorDetails, context?: InstallErrorContext, };

export type InstallApiErrorDetails = { error: string, status?: number, method?: string, url?: string, route?: string, };

export type InstallErrorContext = { operation: string, source_path?: string, target_path?: string, file_path?: string, entry_path?: string, urls: Array<string>, expected_hash?: string, expected_size?: number, project_id?: string, version_id?: string, minecraft_version?: string, loader?: string, java_version?: number, os?: string, arch?: string, };

export type ImportLauncherType = "MultiMC" | "PrismLauncher" | "ATLauncher" | "GDLauncher" | "Curseforge" | "Unknown";

export type ModLoader = "vanilla" | "forge" | "fabric" | "quilt" | "neoforge";

export type WarningPayload = { message: string, };

export type LogPayload = { instance_id: string, } & ({ "type": "log4j" } & Log4jEvent | { "type": "legacy", message: string, });

export type LogEvent = { "type": "log4j" } & Log4jEvent | { "type": "legacy", message: string, };

export type Log4jEvent = { timestamp_millis: number | null, logger_name: string | null, level: string | null, thread_name: string | null, message: string | null, throwable: string | null, };

export type LoadingBar = { loading_bar_uuid: string, message: string, total: number, current: number, bar_type: LoadingBarType, };

export type GameInstance = { id: string, path: string, install_stage: InstanceInstallStage, launcher_feature_version: string, name: string, icon_path: string | null, icon_config: SharedIconConfig | null, game_version: string, protocol_version: number | null, loader: ModLoader, loader_version: string | null, group_ids: Array<string>, synced_options: InstanceSyncedOptions, link: InstanceLink | null, shared_instance: SharedInstanceAttachment | null, quarantined: boolean, update_channel: ReleaseChannel, created: string, modified: string, last_played: string | null, submitted_time_played: number, recent_time_played: number, java_path: string | null, extra_launch_args: Array<string> | null, custom_env_vars: Array<[string, string]> | null, memory: MemorySettings | null, force_fullscreen: boolean | null, game_resolution: WindowSize | null, hooks: Hooks, visible_tabs: InstanceTabVisibility, };

export type InstanceLink = { "type": "modrinth_modpack", project_id: string, version_id: string, } | { "type": "server_project", project_id: string, } | { "type": "server_project_modpack", server_project_id: string, content_project_id: string | null, content_version_id: string, project_id: string | null, version_id: string | null, } | { "type": "imported_modpack", project_id: string | null, version_id: string | null, name: string | null, version_number: string | null, filename: string | null, } | { "type": "modrinth_hosting", server_id: string, instance_ids: Array<string>, active_instance_id: string | null, } | { "type": "shared_instance", modpack_project_id: string | null, modpack_version_id: string | null, };

export type InstanceInstallStage = "installed" | "minecraft_installing" | "pack_installed" | "pack_installing" | "not_installed";

export type SharedInstanceAttachment = { id: string, role: SharedInstanceRole, manager_id: string | null, server_manager_name: string | null, server_manager_icon_url: string | null, linked_user_id: string | null, status: ContentSetSyncStatus, applied_version: number | null, latest_version: number | null, restored_from_remote: boolean, };

export type SharedInstanceRole = "owner" | "member";

export type ContentSetSyncStatus = "unknown" | "up_to_date" | "update_available" | "applying" | "stale" | "not_ready" | "error";

export type InstanceSyncedOptions = { game_options: boolean, command_history: boolean, multiplayer_servers: boolean, creative_hotbars: boolean, screenshots: boolean, resource_packs: boolean, data_packs: boolean, };

export type InstanceTabVisibility = { files: boolean, worlds: boolean, screenshots: boolean, };

export type ReleaseChannel = "release" | "beta" | "alpha";

export type MemorySettings = { maximum: number, };

export type WindowSize = [number, number];

export type Hooks = { pre_launch: string, wrapper: string, post_exit: string, };

export type InstanceRef = { id: string, };

export type InstanceIds = { 
/**
 * A JSON array of ids.
 */
ids: string, };

export type InstanceGroupDefinition = { id: string, name: string, };

export type InstanceGroupMembershipUpdate = { instance_id: string, group_ids: Array<string>, };

export type InstanceGroupRef = { id: string, };

export type NewInstanceGroup = { name: string, };

export type InstanceGroupUpdate = { name: string, };

export type World = { name: string, last_played?: string, icon?: string, display_status: DisplayStatus, } & ({ "type": "singleplayer", path: string, game_mode: SingleplayerGameMode, hardcore: boolean, locked: boolean, } | { "type": "server", index: number, server_id: string, source: ServerSource, address: string, pack_status: ServerPackStatus, project_id?: string, content_kind?: string, });

export type WorldWithInstance = { instance_id: string, name: string, last_played?: string, icon?: string, display_status: DisplayStatus, } & ({ "type": "singleplayer", path: string, game_mode: SingleplayerGameMode, hardcore: boolean, locked: boolean, } | { "type": "server", index: number, server_id: string, source: ServerSource, address: string, pack_status: ServerPackStatus, project_id?: string, content_kind?: string, });

export type WorldDetails = { "type": "singleplayer", path: string, game_mode: SingleplayerGameMode, hardcore: boolean, locked: boolean, } | { "type": "server", index: number, server_id: string, source: ServerSource, address: string, pack_status: ServerPackStatus, project_id?: string, content_kind?: string, };

export type WorldType = "singleplayer" | "server";

export type DisplayStatus = "normal" | "hidden" | "favorite";

export type SingleplayerGameMode = "survival" | "creative" | "adventure" | "spectator";

export type ServerPackStatus = "enabled" | "disabled" | "prompt";

export type ServerSource = "user_synced" | "modpack" | "linked_server_project" | "local_desynced";

export type DesyncServerMode = "keep_in_other_instances" | "remove_from_other_instances";

export type ProtocolVersion = { version: number, legacy: boolean, };

export type ServerStatus = { description?: string | object, players?: ServerPlayers, version?: ServerVersion, favicon?: string, enforcesSecureChat: boolean, ping?: number, };

export type ServerPlayers = { max: number, online: number, sample: Array<ServerGameProfile>, };

export type ServerGameProfile = { id: string, name: string, };

export type ServerVersion = { name: string, protocol: number, legacy: boolean, };

export type RecentWorldsQuery = { limit: number, 
/**
 * A JSON array of statuses; every status when absent.
 */
display_statuses: string | null, };

export type ServerStatusQuery = { address: string, protocol_version: number | null, legacy: boolean, };

export type WorldSaveRef = { id: string, 
/**
 * The world's folder in `saves`.
 */
world: string, };

export type InstanceServerRef = { id: string, index: number, };

export type WorldDisplayStatusUpdate = { world_type: WorldType, 
/**
 * A singleplayer world's folder or a server's address.
 */
world_id: string, display_status: DisplayStatus, };

export type WorldRename = { name: string, };

export type NewInstanceServer = { name: string, address: string, pack_status: ServerPackStatus, project_id: string | null, content_kind: string | null, };

export type ManagedInstanceServer = { name: string, address: string, };

export type InstanceServerUpdate = { name: string, address: string, pack_status: ServerPackStatus, };

export type InstanceServerDesync = { server_id: string, mode: DesyncServerMode, };

export type StoreUsage = { unique_bytes: number, shared_bytes: number, unused_cache_bytes: number, estimated_saved_bytes: number, private_copy_bytes: number, object_count: number, damaged_objects: number, cache_limit_bytes: number, };

export type Settings = { max_concurrent_downloads: number, max_concurrent_writes: number, theme: AppTheme, locale: string, default_page: DefaultPage, collapsed_navigation: boolean, hide_nametag_skins_page: boolean, advanced_rendering: boolean, native_decorations: boolean, toggle_sidebar: boolean, sync_theme_across_devices: boolean, sync_behavior_across_devices: boolean, sync_features_across_devices: boolean, show_files_tab_in_instances: boolean, show_worlds_tab_in_instances: boolean, show_screenshots_tab_in_instances: boolean, show_skin_selector_in_sidebar: boolean, show_jump_in: boolean, always_show_copy_details: boolean, hide_installed_modpacks: boolean, advanced_filters_collapsed: boolean, dismissed_photosensitivity_filter_warning: boolean, friends_active_collapsed: boolean, friends_online_collapsed: boolean, friends_offline_collapsed: boolean, friends_pending_collapsed: boolean, telemetry: boolean, discord_rpc: boolean, personalized_ads: boolean, extra_launch_args: Array<string>, custom_env_vars: Array<[string, string]>, memory: MemorySettings, force_fullscreen: boolean, game_resolution: WindowSize, hide_on_process_start: boolean, refocus_on_game_close: boolean, compact_instance_cards: boolean, show_play_time: boolean, warn_on_unknown_modpacks: boolean, skip_non_essential_warnings: boolean, hooks: Hooks, custom_dir: string | null, prev_custom_dir: string | null, developer_mode: boolean, feature_flags: { [key in FeatureFlag]?: boolean }, skipped_update: string | null, pending_update_toast_for_version: string | null, auto_download_updates: boolean | null, version: number, };

export type AppTheme = "dark" | "light" | "oled" | "retro" | "system";

export type DefaultPage = "Home" | "Library";

export type FeatureFlag = "page_path" | "project_background" | "server_ram_as_bytes_always_on" | "always_show_app_controls" | "show_sync_instances_update_modal" | "pride_fundraiser" | "servers_in_app" | "server_project_qa" | "i18n_debug" | "localhost_sign_in" | "show_version_environment_column";

export type CategoryTag = { name: string, project_type: string, header: string, icon: string, };

export type LoaderTag = { name: string, icon: string, supported_project_types: Array<string>, };

export type GameVersionTag = { version: string, version_type: string, date: string, major: boolean, };

export type LoaderRef = { loader: string, };

export type LoaderManifest = { gameVersions: Array<LoaderGameVersion>, versionGroups?: Array<LoaderVersionGroup>, };

export type LoaderGameVersion = { id: string, stable: boolean, versionGroup?: string, loaders: Array<LoaderVersion>, };

export type LoaderVersion = { id: string, url: string, stable: boolean, };

export type LoaderVersionGroup = { id: string, loaders: Array<LoaderVersion>, };

export type CacheKey = { id: string, };

export type CacheKeys = { ids: Array<string>, cacheBehaviour?: CacheBehaviour, };

export type CacheRead = { cacheBehaviour?: CacheBehaviour, };

export type CacheSearch = { id: string, cacheBehaviour?: CacheBehaviour, };

export type CacheBehaviour = "stale_while_revalidate_skip_offline" | "stale_while_revalidate" | "must_revalidate" | "bypass";

export type CacheValueType = "project" | "project_v3" | "version" | "version_v3" | "user" | "team" | "organization" | "file" | "loader_manifest" | "minecraft_manifest" | "categories" | "report_types" | "loaders" | "game_versions" | "donation_platforms" | "file_hash" | "file_update" | "search_results" | "search_results_v3" | "modpack_files" | "embedded_content_metadata" | "project_versions" | "shared_instance_library";

export type ModrinthProject = { id: string, slug: string | null, project_type: string, team: string, organization: string | null, title: string, description: string, body: string, published: string, updated: string, approved: string | null, status: string, license: ModrinthLicense, client_side: ModrinthSideType, server_side: ModrinthSideType, downloads: number, followers: number, categories: Array<string>, additional_categories: Array<string>, game_versions: Array<string>, loaders: Array<string>, versions: Array<string>, icon_url: string | null, raw_icon_url: string | null, issues_url: string | null, source_url: string | null, wiki_url: string | null, discord_url: string | null, donation_urls: Array<ModrinthDonationLink> | null, gallery: Array<ModrinthGalleryItem>, color: number | null, };

export type ModrinthProjectV3 = { id: string, slug: string | null, [key: string]: JsonValue };

export type ModrinthLicense = { id: string, name: string, url: string | null, };

export type ModrinthGalleryItem = { url: string, raw_url: string, featured: boolean, title: string | null, description: string | null, created: string, ordering: number, };

export type ModrinthDonationLink = { id: string, platform: string, url: string, };

export type ModrinthSideType = "required" | "optional" | "unsupported" | "unknown";

export type ModrinthVersion = { id: string, project_id: string, author_id: string, featured: boolean, name: string, version_number: string, changelog: string | null, changelog_url: string | null, date_published: string, downloads: number, version_type: string, files: Array<ModrinthVersionFile>, dependencies: Array<ModrinthDependency>, game_versions: Array<string>, loaders: Array<string>, };

export type ModrinthVersionFile = { hashes: { [key in string]: string }, url: string, filename: string, primary: boolean, size: number, file_type: ModrinthFileType | null, };

export type ModrinthFileType = "required-resource-pack" | "optional-resource-pack" | "sources-jar" | "dev-jar" | "javadoc-jar" | "signature" | "unknown";

export type ModrinthDependency = { version_id: string | null, project_id: string | null, file_name: string | null, dependency_type: ModrinthDependencyType, };

export type ModrinthDependencyType = "required" | "optional" | "incompatible" | "embedded";

export type ModrinthUser = { id: string, username: string, avatar_url: string | null, bio: string | null, created: string, role: string, badges: number, };

export type ModrinthTeamMember = { team_id: string, user: ModrinthUser, is_owner: boolean, role: string, ordering: number, };

export type ModrinthOrganization = { id: string, slug: string, name: string, team_id: string, description: string, icon_url: string | null, color: number | null, members: Array<ModrinthTeamMember>, };

export type ModrinthSearchResults = { search: string, result: ModrinthSearchResult, };

export type ModrinthSearchResult = { hits: Array<ModrinthSearchEntry>, offset: number, limit: number, total_hits: number, };

export type ModrinthSearchEntry = { project_id: string, project_type: string, slug: string | null, author: string, title: string, description: string, categories: Array<string>, display_categories: Array<string>, versions: Array<string>, downloads: number, follows: number, icon_url: string, date_created: string, date_modified: string, latest_version: string, license: string, client_side: string, server_side: string, gallery: Array<string>, featured_gallery: string | null, color: number | null, };

export type ModrinthSearchResultsV3 = { search: string, result: ModrinthSearchResultV3, };

export type ModrinthSearchResultV3 = { hits: Array<JsonValue>, page: number, hits_per_page: number, total_hits: number, };

export type JsonValue = number | string | boolean | Array<JsonValue> | { [key in string]: JsonValue } | null;

export type Logs = { log_type: LogType, filename: string, age: number, output: CensoredString | null, };

export type LogType = "InfoLog" | "CrashReport";

export type CensoredString = string;

export type LogListing = { 
/**
 * Leaves out each log's output.
 */
clearContents?: boolean, };

export type LogFileRef = { id: string, log_type: LogType, filename: string, };

export type ProcessMetadata = { uuid: string, instance_id: string, instance_path: string, instance_name: string, start_time: string, };

export type ProcessRef = { uuid: string, };

export type JavaVersion = { parsed_version: number, version: string, architecture: string, path: string, };

export type JavaInstallsQuery = { version?: number, };

export type InstanceFileItem = { name: string, type: "file" | "directory" | "symlink", path: string, modified: number, created: number, size: number | null, count: number | null, readOnly: boolean, };

export type InstanceFilePath = { path: string, };

export type InstanceFileWrite = { path: string, 
/**
 * Refuses to replace a file already there.
 */
create_only: boolean, };

export type InstanceFileMove = { source: string, destination: string, };

export type InstanceFileDelete = { path: string, 
/**
 * Deletes a folder that is not empty.
 */
recursive: boolean, };

export type InstanceZipExtract = { path: string, override_conflicts: boolean, dry_run: boolean, };

export type ExtractDryRunResult = { modpack_name: string | null, conflicting_files: Array<string>, };

export type FriendPayload = { "event": "friend_request", from: string, } | { "event": "user_offline", id: string, } | { "event": "status_update", user_status: FriendStatusPayload, } | { "event": "status_sync" } | { "event": "friend_request_rejected", from: string, };

export type FriendStatusPayload = { user_id: string, profile_name: string | null, last_update: string, };

export type OnboardingChecklist = { has_created_instance: boolean, has_logged_into_minecraft: boolean, has_logged_into_modrinth: boolean, show_checklist: boolean, };

export type InstanceGroupsChangedPayload = { instance_ids: Array<string>, };

export type CommandPayload = { "event": "InstallMod", id: string, } | { "event": "InstallVersion", id: string, } | { "event": "InstallModpack", id: string, } | { "event": "InstallServer", id: string, } | { "event": "LaunchInstance", id: string, server: string | null, singleplayer_world: string | null, } | { "event": "InstallSharedInstanceInvite", invite_id: string, } | { "event": "RunMRPack", path: string, } | { "event": "PlayHostingServer", server_id: string, world_id: string, };

export const instances = {
	ListInstances: { method: "GET", path: "/instances", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<GameInstance>>,
	GetInstances: { method: "GET", path: "/instances/many", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, InstanceIds, void, Array<GameInstance>>,
	GetInstance: { method: "GET", path: "/instances/{id}", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<InstanceRef, null, void, GameInstance | null>,
}

export const instanceGroups = {
	ListInstanceGroups: { method: "GET", path: "/instance-groups", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<InstanceGroupDefinition>>,
	CreateInstanceGroup: { method: "POST", path: "/instance-groups", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, NewInstanceGroup, InstanceGroupDefinition>,
	RenameInstanceGroup: { method: "PATCH", path: "/instance-groups/{id}", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<InstanceGroupRef, null, InstanceGroupUpdate, InstanceGroupDefinition>,
	DeleteInstanceGroup: { method: "DELETE", path: "/instance-groups/{id}", auth: "session", dangerous: false, body: "empty", response: "empty" } as Endpoint<InstanceGroupRef, null, void, void>,
	SetInstanceGroupOrder: { method: "PUT", path: "/instance-groups/order", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, Array<string>, void>,
	SetInstanceGroupMemberships: { method: "PUT", path: "/instance-groups/memberships", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, Array<InstanceGroupMembershipUpdate>, void>,
}

export const loadingBars = {
	ListLoadingBars: { method: "GET", path: "/loading-bars", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, { [key in string]: LoadingBar }>,
}

export const worlds = {
	ListRecentWorlds: { method: "GET", path: "/worlds/recent", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, RecentWorldsQuery, void, Array<WorldWithInstance>>,
	GetServerStatus: { method: "GET", path: "/server-status", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, ServerStatusQuery, void, ServerStatus>,
	ListInstanceWorlds: { method: "GET", path: "/instances/{id}/worlds", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<InstanceRef, null, void, Array<World>>,
	SetWorldDisplayStatus: { method: "PUT", path: "/instances/{id}/worlds/display-status", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<InstanceRef, null, WorldDisplayStatusUpdate, void>,
	GetInstanceProtocolVersion: { method: "GET", path: "/instances/{id}/protocol-version", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<InstanceRef, null, void, ProtocolVersion | null>,
	GetSingleplayerWorld: { method: "GET", path: "/instances/{id}/saves/{world}", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<WorldSaveRef, null, void, World>,
	DeleteWorld: { method: "DELETE", path: "/instances/{id}/saves/{world}", auth: "session", dangerous: false, body: "empty", response: "empty" } as Endpoint<WorldSaveRef, null, void, void>,
	RenameWorld: { method: "PUT", path: "/instances/{id}/saves/{world}/name", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<WorldSaveRef, null, WorldRename, void>,
	ResetWorldIcon: { method: "DELETE", path: "/instances/{id}/saves/{world}/icon", auth: "session", dangerous: false, body: "empty", response: "empty" } as Endpoint<WorldSaveRef, null, void, void>,
	BackupWorld: { method: "POST", path: "/instances/{id}/saves/{world}/backups", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<WorldSaveRef, null, void, number>,
	AddInstanceServer: { method: "POST", path: "/instances/{id}/servers", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<InstanceRef, null, NewInstanceServer, number>,
	EnsureManagedInstanceServer: { method: "PUT", path: "/instances/{id}/servers/managed", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<InstanceRef, null, ManagedInstanceServer, void>,
	DesyncInstanceServer: { method: "POST", path: "/instances/{id}/servers/desync", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<InstanceRef, null, InstanceServerDesync, void>,
	EditInstanceServer: { method: "PUT", path: "/instances/{id}/servers/{index}", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<InstanceServerRef, null, InstanceServerUpdate, void>,
	RemoveInstanceServer: { method: "DELETE", path: "/instances/{id}/servers/{index}", auth: "session", dangerous: false, body: "empty", response: "empty" } as Endpoint<InstanceServerRef, null, void, void>,
}

export const contentStore = {
	GetStoreUsage: { method: "GET", path: "/content-store/usage", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, StoreUsage>,
	CleanStore: { method: "POST", path: "/content-store/cleanup", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, number>,
	SetStoreCacheLimit: { method: "PUT", path: "/content-store/cache-limit", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, number, void>,
}

export const tags = {
	GetCategoryTags: { method: "GET", path: "/tags/categories", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<CategoryTag>>,
	GetLoaderTags: { method: "GET", path: "/tags/loaders", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<LoaderTag>>,
	GetGameVersionTags: { method: "GET", path: "/tags/game-versions", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<GameVersionTag>>,
}

export const metadata = {
	GetLoaderManifest: { method: "GET", path: "/metadata/loaders/{loader}", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<LoaderRef, null, void, LoaderManifest>,
}

export const cache = {
	GetCachedProject: { method: "GET", path: "/cache/projects/{id}", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<CacheKey, CacheRead, void, ModrinthProject | null>,
	GetCachedProjects: { method: "POST", path: "/cache/projects", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, CacheKeys, Array<ModrinthProject>>,
	GetCachedProjectVersions: { method: "GET", path: "/cache/projects/{id}/versions", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<CacheKey, CacheRead, void, Array<ModrinthVersion> | null>,
	GetCachedProjectV3: { method: "GET", path: "/cache/v3/projects/{id}", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<CacheKey, CacheRead, void, ModrinthProjectV3 | null>,
	GetCachedProjectsV3: { method: "POST", path: "/cache/v3/projects", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, CacheKeys, Array<ModrinthProjectV3>>,
	GetCachedVersion: { method: "GET", path: "/cache/versions/{id}", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<CacheKey, CacheRead, void, ModrinthVersion | null>,
	GetCachedVersions: { method: "POST", path: "/cache/versions", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, CacheKeys, Array<ModrinthVersion>>,
	GetCachedUser: { method: "GET", path: "/cache/users/{id}", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<CacheKey, CacheRead, void, ModrinthUser | null>,
	GetCachedUsers: { method: "POST", path: "/cache/users", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, CacheKeys, Array<ModrinthUser>>,
	GetCachedTeam: { method: "GET", path: "/cache/teams/{id}", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<CacheKey, CacheRead, void, Array<ModrinthTeamMember> | null>,
	GetCachedOrganization: { method: "GET", path: "/cache/organizations/{id}", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<CacheKey, CacheRead, void, ModrinthOrganization | null>,
	GetCachedSearchResults: { method: "GET", path: "/cache/search", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, CacheSearch, void, ModrinthSearchResults | null>,
	GetCachedSearchResultsV3: { method: "GET", path: "/cache/v3/search", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, CacheSearch, void, ModrinthSearchResultsV3 | null>,
	PurgeCache: { method: "POST", path: "/cache/purge", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, Array<CacheValueType>, void>,
}

export const logs = {
	ListInstanceLogs: { method: "GET", path: "/instances/{id}/logs", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<InstanceRef, LogListing, void, Array<Logs>>,
	GetInstanceLogOutput: { method: "GET", path: "/instances/{id}/logs/{log_type}/{filename}", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<LogFileRef, null, void, CensoredString>,
	DeleteInstanceLog: { method: "DELETE", path: "/instances/{id}/logs/{log_type}/{filename}", auth: "session", dangerous: false, body: "empty", response: "empty" } as Endpoint<LogFileRef, null, void, void>,
	GetLiveLog: { method: "GET", path: "/instances/{id}/live-log", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<InstanceRef, null, void, CensoredString>,
	ClearLiveLog: { method: "DELETE", path: "/instances/{id}/live-log", auth: "session", dangerous: false, body: "empty", response: "empty" } as Endpoint<InstanceRef, null, void, void>,
}

export const processes = {
	ListProcesses: { method: "GET", path: "/processes", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<ProcessMetadata>>,
	ListInstanceProcesses: { method: "GET", path: "/instances/{id}/processes", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<InstanceRef, null, void, Array<ProcessMetadata>>,
	KillProcess: { method: "POST", path: "/processes/{uuid}/kill", auth: "session", dangerous: false, body: "empty", response: "empty" } as Endpoint<ProcessRef, null, void, void>,
}

export const java = {
	GetJavaVersions: { method: "GET", path: "/java/versions", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, { [key in number]: JavaVersion }>,
	SetJavaVersion: { method: "PUT", path: "/java/versions", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, JavaVersion, void>,
	FindJavaInstalls: { method: "GET", path: "/java/installs", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, JavaInstallsQuery, void, Array<JavaVersion>>,
	InstallJava: { method: "POST", path: "/java/installs", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, number, string>,
	GetSystemMemory: { method: "GET", path: "/system/memory", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, number>,
}

export const instanceFiles = {
	ListInstanceFiles: { method: "GET", path: "/instances/{id}/files", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<InstanceRef, InstanceFilePath, void, Array<InstanceFileItem>>,
	ReadInstanceFile: { method: "GET", path: "/instances/{id}/files/content", auth: "session", dangerous: false, body: "empty", response: "raw" } as Endpoint<InstanceRef, InstanceFilePath, void, Blob>,
	WriteInstanceFile: { method: "PUT", path: "/instances/{id}/files/content", auth: "session", dangerous: false, body: "raw", response: "empty" } as Endpoint<InstanceRef, InstanceFileWrite, Blob, void>,
	CreateInstanceDirectory: { method: "POST", path: "/instances/{id}/files/directories", auth: "session", dangerous: false, body: "empty", response: "empty" } as Endpoint<InstanceRef, InstanceFilePath, void, void>,
	MoveInstanceFile: { method: "POST", path: "/instances/{id}/files/move", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<InstanceRef, null, InstanceFileMove, void>,
	DeleteInstanceFile: { method: "DELETE", path: "/instances/{id}/files", auth: "session", dangerous: false, body: "empty", response: "empty" } as Endpoint<InstanceRef, InstanceFileDelete, void, void>,
	ExtractInstanceZip: { method: "POST", path: "/instances/{id}/files/extract", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<InstanceRef, null, InstanceZipExtract, ExtractDryRunResult | null>,
}
