// All response shapes returned by Copal's HTTP API and WebSocket.
// These mirror the Rust structs in apps/core/src/ — keep in sync with changes there.
// Key types: CoreInstance:37, CoreInstanceSummary:54, CoreInstanceEvent:68, CoreStats:82,
// CoreFsOperationKind:228, CoreMetadata:235, CoreMember:247, CoreSyncProfile:258.

export type CoreInstanceStatus = 'offline' | 'starting' | 'running' | 'stopping' | 'crashed'

export type CoreInstanceInstallStatus = 'installing' | 'ready' | 'failed'

export type CoreModLoader = 'vanilla' | 'paper' | 'fabric' | 'forge' | 'neoforge' | 'quilt'

export interface CoreMemory {
	min_mb: number
	max_mb: number
}

export interface CoreSetupStatus {
	paired: boolean
	core_id: string
	dev_mode?: boolean
}

export interface CoreSetupRequest {
	code?: string
	local_setup_secret?: string
	convex_url: string
	auth_jwks_url: string
	owner_user_id: string
	/** JWT audience claim to validate. Defaults to "authenticated" if omitted. */
	auth_audience?: string
	/** Legacy one-time Core realtime credential retained for setup compatibility. */
	realtime_credential?: string
	realtime_url?: string
}

export interface CoreSetupResponse {
	ok: boolean
	core_id: string
}

export type CoreConnectionRejectReason = 'protocol-mismatch' | 'wrong-core'

export interface CoreConnectionHandshakeRequest {
	nonce: string
	protocol: number
	known_core_id?: string | null
}

export interface CoreConnectionHandshakeResponse {
	nonce: string
	ok: boolean
	core_id: string
	protocol: number
	version: string
	reason: CoreConnectionRejectReason | null
}

/** Full detail response from GET /instances/:id */
export interface CoreInstance {
	id: string
	name: string
	game_version: string
	loader: CoreModLoader
	loader_version: string | null
	port: number
	memory: CoreMemory
	java_version: number | null
	install_status: CoreInstanceInstallStatus
	status: CoreInstanceStatus
	installation_id: string | null
	data_dir: string
	created_at: string
	updated_at: string
}

/** Summary response from GET /instances list */
export interface CoreInstanceSummary {
	id: string
	name: string
	game_version: string
	loader: CoreModLoader
	loader_version: string | null
	port: number
	memory: CoreMemory
	install_status: CoreInstanceInstallStatus
	status: CoreInstanceStatus
	installation_id: string | null
	created_at: string
	updated_at: string
}

export type CoreServerInstallationStatus = 'installing' | 'ready' | 'failed'

/** A shared server installation referenced by one or more instances. */
export interface CoreServerInstallation {
	id: string
	game_version: string
	loader: CoreModLoader
	loader_version: string | null
	status: CoreServerInstallationStatus
	error: string | null
	created_at: string
	updated_at: string
}

export type CoreInstanceEvent =
	| { type: 'instance_created'; instance: CoreInstanceSummary }
	| { type: 'instance_updated'; instance: CoreInstanceSummary }
	| { type: 'instance_deleted'; instance_id: string }
	| { type: 'instance_output'; instance_id: string; line: string }
	| { type: 'status_changed'; instance_id: string; status: CoreInstanceStatus }
	| { type: 'macro_output'; instance_id: string; macro_pid: number; line: string }
	| {
			type: 'install_status_changed'
			instance_id: string
			install_status: CoreInstanceInstallStatus
			message?: string | null
	  }
	| { type: 'creation_progress'; instance_id: string; progress: number; message: string }
	| {
			type: 'installation_status_changed'
			installation_id: string
			status: CoreServerInstallationStatus
			message?: string | null
	  }
	| { type: 'fs_changed'; instance_id: string; operation: CoreFsOperationKind; path: string }
	| {
			type: 'sync_profile_updated'
			profile_id: string
			snapshot_id: string
			instance_id: string | null
	  }
	| {
			type: 'sync_event_status_changed'
			profile_id: string
			event_id: string
			status: string
			message?: string | null
	  }

/** GET /instances/:id/stats */
export interface CoreStats {
	cpu_percent: number | null
	memory_mb: number | null
	ram_total_mb: number | null
	player_count: number | null
	uptime_seconds: number | null
	/** Accumulated uptime across all sessions, in seconds. */
	total_uptime_seconds: number | null
	/** Total on-disk size of the instance data directory, in bytes. */
	storage_bytes: number | null
}

/** Entry in `whitelist.json`. */
export interface CoreWhitelistEntry {
	uuid: string
	name: string
}

/** Entry in `ops.json`. */
export interface CoreOpEntry {
	uuid: string
	name: string
	level: number
	bypassesPlayerLimit: boolean
}

/** Entry in `banned-players.json`. */
export interface CoreBanEntry {
	uuid: string
	name: string
	created: string
	source: string
	expires: string
	reason: string
}

/** Entry in `banned-ips.json`. */
export interface CoreIpBanEntry {
	ip: string
	created: string
	source: string
	expires: string
	reason: string
}

/** GET /instances/:id/players — full player-management snapshot. */
export interface CorePlayers {
	/** Names currently online (empty when the server is offline). */
	online: string[]
	whitelist: CoreWhitelistEntry[]
	ops: CoreOpEntry[]
	banned_players: CoreBanEntry[]
	banned_ips: CoreIpBanEntry[]
	whitelist_enabled: boolean
	running: boolean
}

/** Response from enabling RCON on an instance. */
export interface CoreRconEnableResult {
	port: number
	password: string
	restart_required: boolean
}

/** GET /instances/:id/query — live Server List Ping result. */
export interface CoreServerQuery {
	online: boolean
	version_name: string
	protocol: number
	players_online: number
	players_max: number
	sample: string[]
	motd: string
	favicon: string | null
	latency_ms: number
}

/** Scheduled task types. */
export type CoreTaskType = 'backup' | 'restart' | 'command' | 'announce'

/** Single scheduled task from GET /instances/:id/tasks */
export interface CoreScheduledTask {
	id: string
	instance_id: string
	task_type: CoreTaskType
	cron: string
	enabled: boolean
	payload: Record<string, unknown> | null
	created_at: string
	updated_at: string
	last_run_at: string | null
}

/** Request body for POST /instances/:id/tasks */
export interface CoreCreateTaskBody {
	task_type: CoreTaskType
	cron: string
	payload?: Record<string, unknown> | null
}

/** Request body for PATCH /instances/:id/tasks/:task_id */
export interface CoreUpdateTaskBody {
	cron?: string
	enabled?: boolean
	payload?: Record<string, unknown> | null
}

/** Item in GET /instances/:id/mods */
export interface CoreMod {
	id: string | null
	filename: string
	display_name: string | null
	version_number: string | null
	enabled: boolean
	tracked: boolean
	client_side: string | null
	server_side: string | null
	modrinth_project_id: string | null
	modrinth_version_id: string | null
	update_available: boolean | null
}

/** Item in GET/POST /instances/:id/modpack */
export interface CoreModpackManifest {
	id: string
	instance_id: string
	pack_name: string
	pack_version: string
	game_version: string
	loader: string
	loader_version: string | null
	modrinth_project_id: string | null
	modrinth_version_id: string | null
	installed_at: string
}

/** Single file/directory entry from GET /instances/:id/fs */
export interface CoreFsEntry {
	name: string
	path: string
	type: 'file' | 'directory'
	size: number | null
	modified_at: string | null
}

/** Paginated directory listing from GET /instances/:id/fs */
export interface CoreFsListing {
	/** Items on this page — sorted: directories first, then files, both alphabetical. */
	items: CoreFsEntry[]
	/** Total number of pages. */
	total: number
	/** Current page index (0-based). */
	current: number
}

/** Single backup record from GET /instances/:id/backups */
export interface CoreBackup {
	id: string
	name: string
	size_bytes: number
	locked: boolean
	automated: boolean
	hot: boolean
	consistency: 'offline' | 'rcon_flush'
	trigger: 'manual' | 'scheduled' | string
	status: 'done' | 'in_progress'
	created_at: string
}

export interface CoreNetworkStatus {
	direct_api_url: string
	lan_api_url: string | null
	public_api_url: string
	upnp: {
		state: string
		external_ip: string | null
		external_port: number | null
		error: string | null
	}
	cloudflare_tunnel: {
		state: string
		url: string | null
	}
	minecraft_exposure: {
		state: string
		cloudflare_tunnel: boolean
	}
	playit: {
		state: string
		url: string | null
	}
}

export interface CoreUploadSession {
	id: string
	location: string
	offset: number
	length: number
}

/** Active backup operation returned as part of CoreBackupsResponse */
export interface CoreBackupOperation {
	backup_id: string
	operation_type: 'create' | 'restore'
}

/** Full response from GET /instances/:id/backups */
export interface CoreBackupsResponse {
	backups: CoreBackup[]
	active_operations: CoreBackupOperation[]
}

/** Automated backup schedule from GET /instances/:id/backups/schedule */
export interface CoreBackupSchedule {
	enabled: boolean
	cron: string
	retain_count: number
}

/** Structured JSON frames emitted over the WebSocket connection. */
export type CoreWsFrame =
	| { type: 'log'; data: string }
	| { type: 'stats'; data: CoreStats }
	| { type: 'state'; data: { status: CoreInstanceStatus } }

/** Request body for POST /instances */
export interface CoreCreateInstanceBody {
	name: string
	game_version: string
	loader: CoreModLoader
	loader_version?: string
	port: number
	memory?: CoreMemory
}

/** Request body for PATCH /instances/:id */
export interface CorePatchInstanceBody {
	name?: string
	java_version?: number | null
	/** RAM bounds applied on next start. */
	memory?: CoreMemory
	/** Extra JVM args inserted after the memory flags. `null` clears the override. */
	jvm_args?: string | null
	/** Extra server args appended to the launch command. `null` clears the override. */
	server_args?: string | null
}

/** GET /instances/:id/startup — current startup configuration + rendered commands. */
export interface CoreStartupSettings {
	memory: CoreMemory
	java_version: number | null
	jvm_args: string | null
	server_args: string | null
	/** Launch command Core would build with no user overrides. */
	default_command: string
	/** Launch command Core would build with the current overrides applied. */
	effective_command: string
}

/**
 * Request body for POST /instances/:id/change-version.
 * Omitted fields are left unchanged; `loader_version: null` explicitly clears it.
 */
export interface CoreChangeVersionBody {
	game_version?: string
	loader?: string
	loader_version?: string | null
}

/** Request body for POST /instances/:id/backups/schedule */
export interface CoreBackupScheduleBody {
	enabled: boolean
	cron: string
	retain_count: number
}

/** Handle returned by CoreApiClient.uploadFile() for tracking XHR upload progress. */
export interface UploadHandle {
	/** 0–100 progress updates. */
	onProgress: (cb: (percent: number) => void) => void
	/** Resolves when upload is done, rejects on error. */
	done: Promise<void>
	abort: () => void
}

/** One-time download token response from GET /instances/:id/fs/url */
export interface FsDownloadUrlResponse {
	key: string
	expires_in: number
}

/** How to extract a zip archive. */
export type UnzipOption = 'normal' | 'smart' | 'to_dir'

/** Request body for POST /instances/:id/fs/zip */
export interface FsZipRequest {
	sources: string[]
	dest: string
}

/** Request body for POST /instances/:id/fs/copy */
export interface FsCopyRequest {
	sources: string[]
	dest: string
}

/**
 * How a filesystem entry was changed — mirrors Rust's FsOperationKind enum.
 * Unit variants serialize as plain strings; Move serializes as `{ move: { from } }`.
 */
export type CoreFsOperationKind =
	| 'write'
	| 'create'
	| 'delete'
	| 'upload'
	| 'zip'
	| 'unzip'
	| 'copy'
	| { move: { from: string } }

/** Response from GET /core — Core identity and configuration. */
export interface CoreMetadata {
	core_id: string
	name: string
	description?: string | null
	banner?: string | null
	subdomain?: string | null
	setup_mode: 'remote' | 'local'
	run_mode: 'manual' | 'app_open' | 'startup'
	updated_at: string
}

export interface CoreProjectionSyncResult {
	ok: boolean
	status: 'applied' | 'stale' | string
	coreId?: string | null
	projectionRevision?: number | null
	syncedAt?: number | null
}

/** Member record from GET /core/members. */
export interface CoreMember {
	user_id: string
	display_name?: string | null
	role: 'owner' | 'admin' | 'member'
	permission_preset: string
	custom_permissions?: string | null
	status: string
	joined_at: string
	updated_at: string
}

export type CoreAccessRole = 'owner' | 'admin' | 'member'
export type CorePermissionPreset = 'owner' | 'admin' | 'member' | 'viewer' | 'client-only'

export interface CoreAccessMember {
	user_id: string
	display_name?: string | null
	role: CoreAccessRole
	permission_preset: CorePermissionPreset
	custom_permissions?: string | null
	status: string
	joined_at: string
	updated_at: string
	source: 'core' | 'instance'
	role_id?: string | null
	role_snapshot_json?: string | null
	needs_role_reassignment_at?: string | null
}

export interface CoreAccessViewer {
	user_id: string
	role: CoreAccessRole
	permission_preset: CorePermissionPreset
	permissions: string[]
	can_manage_users: boolean
}

export interface CoreAccessResponse {
	members: CoreAccessMember[]
	viewer: CoreAccessViewer
}

export interface CoreAccessUpsertBody {
	user_id: string
	display_name?: string | null
	role: CoreAccessRole
	permission_preset?: CorePermissionPreset | null
	custom_permissions?: Record<string, unknown> | null
}

export interface CoreAccessPatchBody {
	display_name?: string | null
	role?: CoreAccessRole
	permission_preset?: CorePermissionPreset | null
	custom_permissions?: Record<string, unknown> | null
}

export interface CoreRole {
	id: string
	name: string
	description: string
	icon: string
	grants_json: string
	retired_at?: string | null
	created_at: string
	updated_at: string
}

export interface CoreRoleConfiguration {
	roles: CoreRole[]
	require_invite_approval: boolean
}

export interface SaveCoreRoleBody {
	id?: string
	name: string
	description: string
	icon: string
	grants: string[]
}

export type CoreInvitationStatus = 'pending_review' | 'sent' | 'rejected' | 'accepted' | 'declined'

export interface CoreInvitation {
	id: string
	invitee_user_id: string
	invitee_display_name?: string | null
	role_id: string
	role_snapshot_json: string
	inviter_user_id: string
	status: CoreInvitationStatus
	created_at: string
	updated_at: string
	expires_at: string
	responded_at?: string | null
	reviewed_by_user_id?: string | null
}

export interface CreateCoreInvitationBody {
	invitee_user_id: string
	invitee_display_name?: string
	role_id: string
}

export interface CoreActivityLogEntry {
	id: string
	actor_user_id: string
	action: string
	instance_id?: string | null
	target_user_id?: string | null
	metadata_json?: string | null
	created_at: string
}

export interface CoreActivityLogQuery {
	instance_id?: string | null
	actor_user_id?: string | null
	target_user_id?: string | null
	action?: string | null
	min_datetime?: string | null
	max_datetime?: string | null
	limit?: number
	cursor?: string | null
}

export interface CoreActivityLogResponse {
	entries: CoreActivityLogEntry[]
	next_cursor?: string | null
}

/** Sync profile from GET /sync/profiles — Core-side record for a synced modpack instance. */
export interface CoreSyncProfile {
	id: string
	client_profile_id?: string | null
	core_instance_id?: string | null
	name: string
	game_version?: string | null
	loader?: string | null
	sync_enabled: boolean
	created_at: string
	updated_at: string
	last_snapshot_at?: string | null
	current_snapshot_id?: string | null
}

export interface CoreSyncSnapshot {
	id: string
	profile_id: string
	author_user_id: string
	manifest_json: string
	client_only_json?: string | null
	server_manifest_json?: string | null
	notes?: string | null
	created_at: string
	archive_path?: string | null
	archived: boolean
}

export interface CoreSyncEvent {
	id: string
	profile_id: string
	snapshot_id?: string | null
	status: string
	diff_json?: string | null
	message?: string | null
	created_at: string
	applied_at?: string | null
}

export interface CoreSyncSnapshotPublishResult {
	profile: CoreSyncProfile
	snapshot: CoreSyncSnapshot
	event: CoreSyncEvent
}

export interface CoreCreateSyncProfileFromMrpackMetadata {
	name?: string | null
	client_profile_id?: string | null
	core_instance_id?: string | null
	sync_enabled?: boolean | null
	notes?: string | null
}

export interface CoreSyncVersionStatus {
	profile_id: string
	current_snapshot_id?: string | null
	current_snapshot_created_at?: string | null
}
