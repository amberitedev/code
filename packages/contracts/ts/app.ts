// Generated from apps/app by ts-rs. Do not edit.
// Regenerate: cargo test -p theseus_gui -- --ignored write_ts

import type { ApiError, CoreSession, Endpoint, SharedIconConfig, UserFriend, UserStatus } from './contracts'
import type { CommandPayload, DesyncServerMode, EngineEvent, FriendPayload, InstanceGroupsChangedPayload, OnboardingChecklist } from './engine'

export type Boot = { version: string, platform: Platform, os_version: string, 
/**
 * A debug build.
 */
dev: boolean, updates_enabled: boolean, 
/**
 * Hover styles crash older macOS WebKit.
 */
disable_mouseover: boolean, 
/**
 * The dev backend a debug build's dev config names.
 */
backend_url: string | null, };

export type Platform = "windows" | "macos" | "linux";

export type CoreTarget = { core_id: string, account: string, address: string, fingerprint: string, };

export type NewCoreSession = { core_id: string, account: string, address: string, fingerprint: string, ticket: string, };

export type CoreSessionKey = { core_id: string, account: string, };

export type LocalFileQuery = { path: string, };

export type ThumbnailQuery = { path: string, size: number, };

export type RemoteIcon = { source: string, };

export type FileStat = { name: string, size: number, };

export type OpenDialog = { title: string | null, 
/**
 * A folder to start in, or a file name, with or without its folder.
 */
default_path: string | null, directory: boolean, multiple: boolean, filters: Array<DialogFilter>, };

export type SaveDialog = { title: string | null, default_path: string | null, filters: Array<DialogFilter>, };

export type DialogFilter = { name: string, 
/**
 * Without the dot.
 */
extensions: Array<string>, };

export type ExternalLink = { url: string, };

export type LocalPath = { path: string, };

export type AppFolder = { "folder": "launcher_logs" } | { "folder": "app_db_backups" } | { "folder": "synced_options" } | { "folder": "screenshot", instance_id: string, file_name: string, };

export type NewShortcut = { instance_name: string, instance_id: string, output_path: string, server: string | null, singleplayer_world: string | null, };

export type WebviewLog = { level: WebviewLogLevel, message: string, };

export type WebviewLogLevel = "trace" | "debug" | "info" | "warn" | "error";

export type LocalEvent = EngineEvent | AppEvent;

export type AppEvent = { "type": "friend", "payload": FriendPayload } | { "type": "notification", "payload": unknown } | { "type": "onboarding_checklist", "payload": OnboardingChecklist } | { "type": "instance_groups_changed", "payload": InstanceGroupsChangedPayload } | { "type": "command", "payload": Command } | { "type": "game_option_locales_updated" } | { "type": "progress", "payload": { op: string, stage: string | null, done: number, total: number | null, } } | { "type": "op_ended", "payload": { op: string, error: ApiError | null, response: OpResponse | null, } } | { "type": "resync" };

export type Command = CommandPayload | AmberiteCommand;

export type AmberiteCommand = { "event": "ApproveCore", code: string, };

export type RecentIconConfigs = Array<SharedIconConfig>;

export type IdParams = { id: string, };

export type GameSettingsEditorState = { summary_revision: string, canonical_revision: number, catalog_revision: number, total_participating: number, categories: Array<GameSettingCategory>, settings: Array<EditableGameSetting>, };

export type GameSettingCategory = { id: string, is_custom: boolean, };

export type EditableGameSetting = { option_id: string, category_id: string, kind: GameOptionKind, raw_key: string | null, sync_enabled: boolean, canonical_value: GameOptionCanonicalValue | null, value_state: GameOptionValueState, option_revision: number, editor: GameOptionEditor, compatibility: GameOptionCompatibilitySummary, validation_error: GameOptionValidationError | null, controlled: boolean, };

export type GameOptionKind = "vanilla" | "external";

export type GameOptionCanonicalValue = { "type": "bool", "value": boolean } | { "type": "integer", "value": number } | { "type": "decimal", "value": string } | { "type": "enum", "value": string } | { "type": "text", "value": string } | { "type": "string_list", "value": Array<string> } | { "type": "key_binding", "value": string } | { "type": "external_raw", "value": string };

export type GameOptionValueState = "canonical" | "uniform_local" | "mixed" | "unset" | "invalid";

export type GameOptionEditor = { type: "bool" | "integer" | "decimal" | "enum" | "text" | "string_list" | "key_binding" | "external_raw", min: number | null, max: number | null, step: number | null, unit: string | null, choices: Array<GameOptionEditorChoice>, };

export type GameOptionEditorChoice = { value: string, };

export type GameOptionCompatibilitySummary = { total_participating: number, will_receive: number, write_now: number, left_local: number, buckets: Array<GameOptionCompatibilityBucket>, };

export type GameOptionCompatibilityBucket = { instance_count: number, write_keys: Array<string>, eventual_keys: Array<string>, game_versions: Array<string>, status: GameOptionCompatibilityStatus, mapping: GameOptionMappingKind | null, reason: GameOptionCompatibilityReason | null, };

export type GameOptionCompatibilityStatus = "ready" | "deferred" | "waiting_for_file" | "waiting_for_base" | "not_available" | "unsupported_value" | "unmappable" | "catalog_uncovered" | "controlled" | "degraded";

export type GameOptionMappingKind = "direct" | "legacy" | "migrated" | "lossless" | "lossy";

export type GameOptionCompatibilityReason = "launcher_controlled" | "catalog_uncovered" | "inspection_failed" | "waiting_for_options_file" | "unsupported_value" | "migrates_on_write" | "waiting_for_compatible_base" | "missing_setting";

export type GameOptionValidationError = "missing_value" | "no_compatible_instances" | "invalid_value" | "changed_since_opened";

export type UpdateGameSettingsRequest = { expected_summary_revision: string, expected_canonical_revision: number, expected_catalog_revision: number, changes: Array<GameSettingChange>, };

export type GameSettingChange = { option_id: string, base_option_revision: number, sync_enabled?: boolean, canonical_value?: GameOptionCanonicalValue | null, };

export type SaveGameSettingsResult = { state: GameSettingsEditorState | null, applied: number, migrated: number, deferred: number, unsupported: number, failed: number, conflicts: Array<string>, };

export type GameOptionsSourceCandidate = { source_id: string, instance_id: string, name: string, icon_path: string | null, game_version: string | null, eligible: boolean, disabled_reason: GameOptionsSourceDisabledReason | null, recognized_setting_count: number, custom_setting_count: number, };

export type GameOptionsSourceDisabledReason = "installing_or_updating" | "unsupported_version" | "missing_options_file" | "no_syncable_settings" | "unreadable_options_file";

export type GameSettingLabelsRequest = { instance_id?: string, locale: string, option_ids: Array<string>, refresh_sources: boolean, };

export type GameSettingLocaleLabels = { settings: { [key in string]: GameSettingLocaleLabel }, };

export type GameSettingLocaleLabel = { label: string, choices: { [key in string]: string }, source: GameSettingSource | null, };

export type GameSettingSource = { instance_id: string, file_name: string, file_path: string, project: GameSettingSourceProject | null, };

export type GameSettingSourceProject = { id: string, title: string, icon_url: string | null, };

export type SyncedOption = "game_options" | "command_history" | "multiplayer_servers" | "creative_hotbars" | "screenshots" | "resource_packs" | "data_packs";

export type InstanceSyncedOptionUpdate = { option: SyncedOption, enabled: boolean, resolution?: SyncedOptionJoinResolution, };

export type SyncedOptionJoinResolution = "use_synced" | "use_instance";

export type SyncedOptionQuery = { option: SyncedOption, };

export type SyncedOptionJoinPreview = { action: SyncedOptionJoinAction, };

export type SyncedOptionJoinAction = "seed_shared" | "attach" | "merge" | "requires_resolution";

export type SyncedOptionsOverview = { global_options: GlobalSyncedOptions, capabilities: Array<SyncedOptionCapability>, };

export type SyncedOptionCapability = { option: SyncedOption, supported: boolean, disabled_reason: string | null, };

export type GlobalSyncedOptions = { game_options: boolean, command_history: boolean, multiplayer_servers: boolean, creative_hotbars: boolean, screenshots: boolean, resource_packs: boolean, data_packs: boolean, };

export type GlobalSyncedOptionUpdate = { option: SyncedOption, enabled: boolean, base_instance_id?: string, };

export type SyncedOptionsRebuild = { instance_id?: string, };

export type CommandHistory = { contents: string, };

export type SyncedServer = { id: string, name: string, address: string, accept_textures?: boolean | null, };

export type SyncedPackType = "resourcepack" | "datapack";

export type SyncedPackQuery = { project_type: SyncedPackType, };

export type SyncedPacks = Array<unknown>;

export type NewSyncedPack = { path: string, project_type: SyncedPackType, game_versions: Array<string>, };

export type SyncedPackUpdate = { enabled: boolean, };

export type InstancePack = { project_path: string, };

export type InstancePackParams = { id: string, pack_id: string, };

export type PackSyncPreview = { pack: unknown, instances: Array<PackSyncTarget>, };

export type PackSyncTarget = { instance_id: string, name: string, game_version: string, compatible: boolean, participating: boolean, };

export type DesyncQuery = { mode: DesyncServerMode, };

export type ScreenshotFile = { id: string, instance_id: string, instance_name: string, file_name: string, created_at: string, modified_at: number, group_id: string | null, path: string, };

export type ScreenshotKey = { instance_id: string, file_name: string, };

export type ScreenshotEdit = { instance_id: string, file_name: string, mode: ScreenshotEditSaveMode, };

export type ScreenshotEditSaveMode = "create_copy" | "replace_edit";

export type ScreenshotExport = { keys: Array<ScreenshotKey>, export_path: string, };

export type ScreenshotMove = { keys: Array<ScreenshotKey>, target_instance_id: string, };

export type ScreenshotGroup = { id: string, name: string, };

export type NewScreenshotGroup = { name: string, screenshot_ids: Array<string>, };

export type ScreenshotGroupUpdate = { name: string, };

export type ScreenshotGroupMembershipUpdate = { screenshot_id: string, group_id?: string | null, };

export type ScreenshotGroupImport = { id: string, name: string, screenshot_ids: Array<string>, };

export type Cape = { 
/**
 * An identifier for this cape, potentially unique to the owning player.
 */
id: string, 
/**
 * The name of the cape.
 */
name: string, 
/**
 * The URL of the cape PNG texture.
 */
texture: string, 
/**
 * Whether the cape is currently equipped in the Minecraft profile of its corresponding
 * player.
 */
is_equipped: boolean, };

export type Skin = { 
/**
 * A key used to recognize this skin texture.
 */
texture_key: string, 
/**
 * The name of the skin, if available.
 */
name?: string | null, 
/**
 * The section this skin should be grouped under, if available.
 */
section?: string | null, 
/**
 * The variant of the skin model.
 */
variant: SkinModel, 
/**
 * The UUID of the cape that this skin uses, if any.
 *
 * If `None`, this skin uses no cape.
 */
cape_id?: string | null, 
/**
 * The URL of the skin PNG texture. Can also be a data URL.
 */
texture: string, 
/**
 * The source of the skin, which represents how the app knows about it.
 */
source: SkinSource, 
/**
 * Whether the skin is currently equipped in the Minecraft profile of its corresponding
 * player.
 */
is_equipped: boolean, };

export type SkinModel = "CLASSIC" | "SLIM" | "UNKNOWN";

export type SkinSource = "default" | "custom_external" | "custom";

export type NewCustomSkin = { texture_blob: Array<number>, variant: SkinModel, cape?: Cape | null, };

export type CustomSkinSave = { skin: Skin, texture_blob: Array<number>, variant: SkinModel, cape?: Cape | null, replace_texture: boolean, };

export type SkinFlushQuery = { profile_id?: string | null, };

export type UrlOrBlob = string | Array<number>;

export type DebugExport = { op: string, path: string, };

export type ExternalSave = { op: string, path: string, url: string, headers: { [key in string]: string }, };

export type LocalUpload = { op: string, url: string, headers: { [key in string]: string }, path: string, 
/**
 * The name a multipart upload sends, the name of `path` by default.
 */
file_name: string | null, 
/**
 * Sends the file as this multipart form field rather than as the raw body.
 */
multipart_field: string | null, };

export type InstanceFileSave = { op: string, instance_id: string, 
/**
 * The file's path in the instance.
 */
path: string, destination: string, };

export type OpResponse = { status: number, body: string, };

export type AvailableUpdate = { version: string, };

export type UpdateDownload = { op: string, };

export type RestartAfterUpdate = { restart: boolean, };

export type MinecraftAccountSummary = { profile: MinecraftProfile, expires: string, active: boolean, };

export type MinecraftProfile = { 
/**
 * The UUID of the player.
 */
id: string, 
/**
 * The username of the player.
 */
name: string, 
/**
 * The skins the player is known to have.
 *
 * As of 2025-04-08, in the production Mojang profile endpoint every
 * player has a single skin.
 */
skins: Array<MinecraftSkin>, 
/**
 * The capes the player is known to have.
 */
capes: Array<MinecraftCape>, };

export type MinecraftSkin = { 
/**
 * The UUID of this skin object.
 *
 * As of 2025-04-08, in the production Mojang profile endpoint this UUID
 * changes every time the player changes their skin, even if the skin
 * texture is the same as before.
 */
id: string, 
/**
 * The selection state of the skin.
 *
 * As of 2025-04-08, in the production Mojang profile endpoint this
 * is always `ACTIVE`, as only a single skin representing the current
 * skin is returned.
 */
state: MinecraftCharacterExpressionState, 
/**
 * The URL to the skin texture.
 *
 * As of 2025-04-08, in the production Mojang profile endpoint the file
 * name for this URL is a hash of the skin texture, so that different
 * players using the same skin texture will share a texture URL.
 */
url: string, 
/**
 * A hash of the skin texture.
 *
 * As of 2025-04-08, in the production Mojang profile endpoint this
 * is always set and the same as the file name of the skin texture URL.
 */
textureKey: string | null, 
/**
 * The player model variant this skin is for.
 */
variant: SkinModel, 
/**
 * User-friendly name for the skin.
 *
 * As of 2025-04-08, in the production Mojang profile endpoint this is
 * only set if the player has not set a custom skin, and this skin object
 * is therefore the default skin for the player's UUID.
 */
alias: string | null, };

export type MinecraftCape = { 
/**
 * The UUID of the cape.
 */
id: string, 
/**
 * The selection state of the cape.
 */
state: MinecraftCharacterExpressionState, 
/**
 * The URL to the cape texture.
 */
url: string, 
/**
 * The user-friendly name for the cape.
 */
alias: string, };

export type MinecraftCharacterExpressionState = "ACTIVE" | "INACTIVE" | "UNKNOWN";

export type MinecraftAccountParams = { id: string, };

export type ModrinthCredentials = { session: string, expires: string, user_id: string, active: boolean, };

export type ModrinthSignIn = { flow: ModrinthAuthFlow, add_account: boolean, };

export type ModrinthAuthFlow = "sign-in" | "sign-up";

export type ServerJoin = { instance_id: string, address: string, };

export type SingleplayerJoin = { instance_id: string, 
/**
 * The world's folder in `saves`.
 */
world: string, };

export const health = {
	checkHealth: { method: "GET", path: "/health", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, null, void, void>,
}

export const boot = {
	getBoot: { method: "GET", path: "/app/boot", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Boot>,
}

export const pairedCores = {
	connectCore: { method: "POST", path: "/app/cores/connect", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, CoreTarget, string>,
	signInToCore: { method: "POST", path: "/app/cores/sessions", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, NewCoreSession, CoreSession>,
	signOutOfCore: { method: "DELETE", path: "/app/cores/sessions", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, CoreSessionKey, void, void>,
	checkCoreSession: { method: "GET", path: "/app/cores/sessions", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, CoreSessionKey, void, boolean>,
}

export const events = {
	eventSocket: { method: "GET", path: "/events", auth: "none", dangerous: false, body: "empty", response: "upgrade" } as Endpoint<null, null, void, { receive: LocalEvent, send: null }>,
}

export const localFiles = {
	getLocalFile: { method: "GET", path: "/app/file", auth: "none", dangerous: false, body: "empty", response: "stream" } as Endpoint<null, LocalFileQuery, void, Response>,
	getThumbnail: { method: "GET", path: "/app/thumbnail", auth: "none", dangerous: false, body: "empty", response: "stream" } as Endpoint<null, ThumbnailQuery, void, Response>,
	cacheRemoteIcon: { method: "POST", path: "/app/icons/remote", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, RemoteIcon, string>,
	readLocalFile: { method: "GET", path: "/app/fs/read", auth: "none", dangerous: false, body: "empty", response: "stream" } as Endpoint<null, LocalFileQuery, void, Response>,
	statLocalFile: { method: "GET", path: "/app/fs/stat", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, LocalFileQuery, void, FileStat>,
}

export const desktop = {
	pickFiles: { method: "POST", path: "/app/dialog/open", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, OpenDialog, Array<string> | null>,
	pickSavePath: { method: "POST", path: "/app/dialog/save", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, SaveDialog, string | null>,
	openUrl: { method: "POST", path: "/app/opener/url", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, ExternalLink, void>,
	openPath: { method: "POST", path: "/app/opener/path", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, LocalPath, void>,
	revealPath: { method: "POST", path: "/app/opener/reveal", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, LocalPath, void>,
	openFolder: { method: "POST", path: "/app/folders/open", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, AppFolder, void>,
	createShortcut: { method: "POST", path: "/app/shortcuts", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, NewShortcut, string>,
	checkNetworkMetered: { method: "GET", path: "/app/network/metered", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, boolean>,
	logFromWebview: { method: "POST", path: "/app/webview-log", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, Array<WebviewLog>, void>,
}

export const library = {
	listRecentIconConfigs: { method: "GET", path: "/app/library/recent-icon-configs", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, RecentIconConfigs>,
}

export const gameOptions = {
	listGameOptionsSources: { method: "GET", path: "/app/game-options/sources", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<GameOptionsSourceCandidate>>,
	getSyncedGameOptions: { method: "GET", path: "/app/game-options", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, GameSettingsEditorState>,
	previewSyncedGameOptions: { method: "POST", path: "/app/game-options/preview", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, UpdateGameSettingsRequest, GameSettingsEditorState>,
	saveSyncedGameOptions: { method: "PATCH", path: "/app/game-options", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, UpdateGameSettingsRequest, SaveGameSettingsResult>,
	getInstanceGameOptions: { method: "GET", path: "/app/instances/{id}/game-options", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<IdParams, null, void, GameSettingsEditorState>,
	previewInstanceGameOptions: { method: "POST", path: "/app/instances/{id}/game-options/preview", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<IdParams, null, UpdateGameSettingsRequest, GameSettingsEditorState>,
	saveInstanceGameOptions: { method: "PATCH", path: "/app/instances/{id}/game-options", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<IdParams, null, UpdateGameSettingsRequest, SaveGameSettingsResult>,
	getGameSettingLabels: { method: "POST", path: "/app/game-options/labels", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, GameSettingLabelsRequest, GameSettingLocaleLabels>,
}

export const syncedOptions = {
	setInstanceSyncedOption: { method: "PUT", path: "/app/instances/{id}/synced-options", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<IdParams, null, InstanceSyncedOptionUpdate, void>,
	previewSyncedOptionJoin: { method: "GET", path: "/app/instances/{id}/synced-options/join-preview", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<IdParams, SyncedOptionQuery, void, SyncedOptionJoinPreview>,
	getInstanceSyncedOptions: { method: "GET", path: "/app/instances/{id}/synced-options", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<IdParams, null, void, SyncedOptionsOverview>,
	getGlobalSyncedOptions: { method: "GET", path: "/app/synced-options", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, GlobalSyncedOptions>,
	getInitializedSyncedOptions: { method: "GET", path: "/app/synced-options/initialized", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, GlobalSyncedOptions>,
	setGlobalSyncedOption: { method: "PUT", path: "/app/synced-options", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, GlobalSyncedOptionUpdate, GlobalSyncedOptions>,
	rebuildSyncedOptions: { method: "POST", path: "/app/synced-options/rebuild", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, SyncedOptionsRebuild, void>,
	getCommandHistory: { method: "GET", path: "/app/synced-options/command-history", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, string>,
	setCommandHistory: { method: "PUT", path: "/app/synced-options/command-history", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, CommandHistory, string>,
	listSyncedServers: { method: "GET", path: "/app/synced-servers", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<SyncedServer>>,
	updateSyncedServer: { method: "PUT", path: "/app/synced-servers", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, SyncedServer, void>,
	removeSyncedServer: { method: "DELETE", path: "/app/synced-servers/{id}", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<IdParams, null, void, void>,
}

export const syncedPacks = {
	listSyncedPacks: { method: "GET", path: "/app/synced-packs", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, SyncedPackQuery, void, SyncedPacks>,
	uploadSyncedPack: { method: "POST", path: "/app/synced-packs", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, NewSyncedPack, void>,
	setSyncedPackEnabled: { method: "PATCH", path: "/app/synced-packs/{id}", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<IdParams, null, SyncedPackUpdate, void>,
	removeSyncedPack: { method: "DELETE", path: "/app/synced-packs/{id}", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<IdParams, null, void, void>,
	previewPackSync: { method: "GET", path: "/app/instances/{id}/synced-packs/preview", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<IdParams, InstancePack, void, PackSyncPreview>,
	syncPack: { method: "POST", path: "/app/instances/{id}/synced-packs", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<IdParams, null, InstancePack, void>,
	desyncPack: { method: "DELETE", path: "/app/instances/{id}/synced-packs/{pack_id}", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<InstancePackParams, DesyncQuery, void, void>,
}

export const screenshots = {
	listInstanceScreenshots: { method: "GET", path: "/app/instances/{id}/screenshots", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<IdParams, null, void, Array<ScreenshotFile>>,
	listScreenshots: { method: "GET", path: "/app/screenshots", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<ScreenshotFile>>,
	listSyncedScreenshots: { method: "GET", path: "/app/screenshots/synced", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<ScreenshotFile>>,
	saveEditedScreenshot: { method: "POST", path: "/app/screenshots/edits", auth: "none", dangerous: false, body: "raw", response: "json" } as Endpoint<null, ScreenshotEdit, Blob, ScreenshotFile>,
	deleteScreenshots: { method: "DELETE", path: "/app/screenshots", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, Array<ScreenshotKey>, void>,
	exportScreenshots: { method: "POST", path: "/app/screenshots/export", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, ScreenshotExport, void>,
	moveScreenshots: { method: "POST", path: "/app/screenshots/move", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, ScreenshotMove, Array<ScreenshotKey>>,
	listScreenshotGroups: { method: "GET", path: "/app/screenshot-groups", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<ScreenshotGroup>>,
	createScreenshotGroup: { method: "POST", path: "/app/screenshot-groups", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, NewScreenshotGroup, ScreenshotGroup>,
	renameScreenshotGroup: { method: "PATCH", path: "/app/screenshot-groups/{id}", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<IdParams, null, ScreenshotGroupUpdate, ScreenshotGroup>,
	deleteScreenshotGroup: { method: "DELETE", path: "/app/screenshot-groups/{id}", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<IdParams, null, void, void>,
	setScreenshotGroupMemberships: { method: "PUT", path: "/app/screenshot-groups/memberships", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, Array<ScreenshotGroupMembershipUpdate>, void>,
	importScreenshotGroups: { method: "POST", path: "/app/screenshot-groups/import", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, Array<ScreenshotGroupImport>, void>,
}

export const friends = {
	listFriends: { method: "GET", path: "/app/friends", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<UserFriend>>,
	listFriendStatuses: { method: "GET", path: "/app/friend-statuses", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<UserStatus>>,
	addFriend: { method: "POST", path: "/app/friends/{id}", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<IdParams, null, void, void>,
	removeFriend: { method: "DELETE", path: "/app/friends/{id}", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<IdParams, null, void, void>,
}

export const skins = {
	listCapes: { method: "GET", path: "/app/skins/capes", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<Cape>>,
	listSkins: { method: "GET", path: "/app/skins", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<Skin>>,
	saveCustomSkin: { method: "POST", path: "/app/skins", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, CustomSkinSave, Skin>,
	removeCustomSkin: { method: "DELETE", path: "/app/skins", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, Skin, void>,
	setCustomSkinOrder: { method: "PUT", path: "/app/skins/order", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, Array<string>, void>,
	addAndEquipCustomSkin: { method: "POST", path: "/app/skins/equipped", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, NewCustomSkin, Skin>,
	equipSkin: { method: "PUT", path: "/app/skins/equipped", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, Skin, void>,
	unequipSkin: { method: "DELETE", path: "/app/skins/equipped", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, null, void, void>,
	flushPendingSkinChange: { method: "POST", path: "/app/skins/flush", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, SkinFlushQuery, void, void>,
	normalizeSkinTexture: { method: "POST", path: "/app/skins/normalize", auth: "none", dangerous: false, body: "json", response: "raw" } as Endpoint<null, null, UrlOrBlob, Blob>,
}

export const ops = {
	listOps: { method: "GET", path: "/app/ops", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<string>>,
	cancelOp: { method: "DELETE", path: "/app/ops/{id}", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<IdParams, null, void, void>,
	exportDebugInfo: { method: "POST", path: "/app/ops/debug-export", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, DebugExport, void>,
	saveExternalFile: { method: "POST", path: "/app/ops/external-save", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, ExternalSave, void>,
	uploadLocalFile: { method: "POST", path: "/app/ops/upload", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, LocalUpload, void>,
	saveInstanceFileAs: { method: "POST", path: "/app/ops/instance-file-save", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, InstanceFileSave, void>,
}

export const updates = {
	checkForUpdate: { method: "POST", path: "/app/updates/check", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, AvailableUpdate | null>,
	getUpdateSize: { method: "GET", path: "/app/updates/size", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, number | null>,
	downloadUpdate: { method: "POST", path: "/app/ops/update-download", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, UpdateDownload, void>,
	discardDownloadedUpdate: { method: "DELETE", path: "/app/updates/downloaded", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, null, void, void>,
	setRestartAfterUpdate: { method: "PUT", path: "/app/updates/restart-after", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, RestartAfterUpdate, void>,
	restartApp: { method: "POST", path: "/app/restart", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, null, void, void>,
}

export const localAccounts = {
	checkMinecraftReachable: { method: "GET", path: "/app/accounts/minecraft/reachable", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, null, void, void>,
	signInToMinecraft: { method: "POST", path: "/app/accounts/minecraft/login", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, MinecraftAccountSummary>,
	listMinecraftAccounts: { method: "GET", path: "/app/accounts/minecraft", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<MinecraftAccountSummary>>,
	getDefaultMinecraftAccount: { method: "GET", path: "/app/accounts/minecraft/default", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, string | null>,
	setDefaultMinecraftAccount: { method: "PUT", path: "/app/accounts/minecraft/default", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, string, void>,
	removeMinecraftAccount: { method: "DELETE", path: "/app/accounts/minecraft/{id}", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<MinecraftAccountParams, null, void, void>,
	signInToModrinth: { method: "POST", path: "/app/accounts/modrinth/login", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, ModrinthSignIn, ModrinthCredentials>,
	cancelModrinthSignIn: { method: "DELETE", path: "/app/accounts/modrinth/login", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, null, void, void>,
	signOutOfModrinth: { method: "POST", path: "/app/accounts/modrinth/logout", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, null, void, void>,
	getActiveModrinthAccount: { method: "GET", path: "/app/accounts/modrinth/active", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, ModrinthCredentials | null>,
	listModrinthAccounts: { method: "GET", path: "/app/accounts/modrinth", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<ModrinthCredentials>>,
	setActiveModrinthAccount: { method: "PUT", path: "/app/accounts/modrinth/active", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, string, void>,
	removeModrinthAccount: { method: "DELETE", path: "/app/accounts/modrinth/{id}", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<IdParams, null, void, void>,
}

export const launch = {
	joinServer: { method: "POST", path: "/app/launch/server", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, ServerJoin, void>,
	joinSingleplayerWorld: { method: "POST", path: "/app/launch/singleplayer", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, SingleplayerJoin, void>,
}
