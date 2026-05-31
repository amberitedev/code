import type {
	CoreBackup,
	CoreFsEntry,
	CoreInstance,
	CoreInstanceStatus,
	CoreMod,
	CoreStats,
	CoreWsConnection,
} from '@amberite/amberite-api'
import type { Archon } from '@modrinth/api-client'
import type { ContentItem, FileItem, LogLine } from '@modrinth/ui'
import type { Stats } from '@modrinth/utils'
import type { ComputedRef, Ref } from 'vue'

export type CoreServerViewData = Archon.Servers.v0.Server
export type CoreServerContext = {
	instanceId: ComputedRef<string>
	server: ComputedRef<CoreServerViewData | null>
	statsData: Ref<CoreStats | null>
	stats: Ref<Stats>
	powerState: Ref<Archon.Websocket.v0.PowerState>
	logLines: Ref<LogLine[]>
	refreshServer: () => Promise<void>
	refreshStats: () => Promise<void>
	sendCommand: (command: string) => Promise<void>
	startServer: () => Promise<void>
	stopServer: () => Promise<void>
	restartServer: () => Promise<void>
	killServer: () => Promise<void>
}

export const coreServerContextKey = Symbol('core-server-context')

export function toHostingServer(instance: CoreInstance): CoreServerViewData {
	return {
		server_id: instance.id,
		name: instance.name,
		owner_id: 'local',
		net: {
			ip: '127.0.0.1',
			port: instance.port,
			domain: `127.0.0.1:${instance.port}`,
		},
		game: 'Minecraft',
		backup_quota: 0,
		used_backup_quota: 0,
		status: instance.install_status === 'failed' ? 'broken' : 'available',
		suspension_reason: null,
		loader: toHostingLoader(instance.loader),
		loader_version: instance.loader_version,
		mc_version: instance.game_version,
		upstream: null,
		sftp_username: '',
		sftp_password: '',
		sftp_host: '',
		datacenter: 'Local Core',
		notices: [],
		node: null,
		flows: { intro: false },
		is_medal: false,
	}
}

export function toHostingPowerState(status: CoreInstanceStatus): Archon.Websocket.v0.PowerState {
	if (status === 'offline') return 'stopped'
	return status
}

export function toStats(stats: CoreStats | null | undefined): Stats {
	const ramUsageBytes = (stats?.memory_mb ?? 0) * 1024 * 1024
	const ramTotalBytes = (stats?.ram_total_mb ?? 1) * 1024 * 1024
	return {
		current: {
			cpu_percent: stats?.cpu_percent ?? 0,
			ram_usage_bytes: ramUsageBytes,
			ram_total_bytes: Math.max(ramTotalBytes, 1),
			storage_usage_bytes: 0,
			storage_total_bytes: 0,
		},
		past: {
			cpu_percent: 0,
			ram_usage_bytes: 0,
			ram_total_bytes: Math.max(ramTotalBytes, 1),
			storage_usage_bytes: 0,
			storage_total_bytes: 0,
		},
		graph: {
			cpu: [stats?.cpu_percent ?? 0],
			ram: [Math.round((ramUsageBytes / Math.max(ramTotalBytes, 1)) * 100)],
		},
	}
}

export function toLogLine(raw: string): LogLine {
	const lower = raw.toLowerCase()
	const level = lower.includes('error')
		? 'error'
		: lower.includes('warn')
			? 'warn'
			: lower.includes('debug')
				? 'debug'
				: lower.includes('trace')
					? 'trace'
					: 'info'
	return { text: raw, level }
}

export function toContentItem(mod: CoreMod): ContentItem {
	const id = mod.id ?? mod.filename
	return {
		id,
		file_name: mod.filename,
		file_path: `mods/${mod.filename}`,
		size: undefined,
		project_type: 'mod',
		has_update: mod.update_available === true,
		update_version_id: null,
		project: mod.display_name
			? {
					id: mod.modrinth_project_id ?? id,
					slug: mod.modrinth_project_id ?? id,
					title: mod.display_name,
					icon_url: null,
				}
			: undefined,
		version: {
			id: mod.modrinth_version_id ?? mod.filename,
			version_number: mod.version_number ?? 'Unknown',
			file_name: mod.filename,
		},
		owner: undefined,
		enabled: mod.enabled,
		environment: mod.server_side ?? mod.client_side ?? undefined,
		pack_client_retained: false,
		pack_client_depends: false,
	}
}

export function toFileItem(entry: CoreFsEntry): FileItem {
	const modified = entry.modified_at ? new Date(entry.modified_at).getTime() : Date.now()
	return {
		name: entry.name,
		type: entry.type === 'directory' ? 'directory' : 'file',
		path: entry.path,
		modified,
		created: modified,
		size: entry.size ?? undefined,
	}
}

export function toBackupItem(backup: CoreBackup): Archon.BackupsQueue.v1.BackupQueueBackup {
	return {
		id: backup.id,
		name: backup.name,
		created_at: backup.created_at,
		status: backup.status === 'done' ? 'done' : 'in_progress',
		locked: backup.locked,
		automated: backup.automated,
		history: [],
	}
}

export function appendSocketListeners(
	connection: CoreWsConnection,
	listeners: Array<() => void>,
	callbacks: {
		onLog: (line: string) => void
		onStats: (stats: CoreStats) => void
		onState: (status: CoreInstanceStatus) => void
		onOpen: () => void
		onClose: () => void
		onError: () => void
	},
) {
	listeners.push(connection.on('log', callbacks.onLog))
	listeners.push(connection.on('stats', callbacks.onStats))
	listeners.push(connection.on('state', callbacks.onState))
	listeners.push(connection.on('open', callbacks.onOpen))
	listeners.push(connection.on('close', callbacks.onClose))
	listeners.push(connection.on('error', callbacks.onError))
}

function toHostingLoader(loader: CoreInstance['loader']): Archon.Servers.v0.Loader {
	const map: Record<CoreInstance['loader'], Archon.Servers.v0.Loader> = {
		vanilla: 'Vanilla',
		paper: 'Paper',
		fabric: 'Fabric',
		forge: 'Forge',
		neoforge: 'NeoForge',
		quilt: 'Quilt',
	}
	return map[loader]
}
