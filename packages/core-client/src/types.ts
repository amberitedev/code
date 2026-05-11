// All response shapes returned by Amberite Core's HTTP API and WebSocket.
// These mirror the Rust structs in apps/core/src/ — keep in sync with changes there.

export type CoreInstanceStatus = 'offline' | 'starting' | 'running' | 'stopping' | 'crashed'

export type CoreModLoader = 'vanilla' | 'paper' | 'fabric' | 'forge' | 'neoforge' | 'quilt'

export interface CoreMemory {
	min_mb: number
	max_mb: number
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
	status: CoreInstanceStatus
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
	update_available: boolean | null
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
