// All response shapes returned by Amberite Core's HTTP API and WebSocket.
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
	created_at: string
	updated_at: string
}

export type CoreInstanceEvent =
	| { type: 'instance_created'; instance: CoreInstanceSummary }
	| { type: 'instance_updated'; instance: CoreInstanceSummary }
	| { type: 'instance_deleted'; instance_id: string }
	| { type: 'status_changed'; instance_id: string; status: CoreInstanceStatus }
	| {
			type: 'install_status_changed'
			instance_id: string
			install_status: CoreInstanceInstallStatus
			message?: string | null
	  }
	| { type: 'creation_progress'; instance_id: string; progress: number; message: string }
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
	status: 'done' | 'in_progress'
	created_at: string
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
	| 'rename'
	| 'read'
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
