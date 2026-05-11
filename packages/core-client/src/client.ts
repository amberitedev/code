/**
 * CoreApiClient — primary entry point for all Amberite Core API calls.
 *
 * Instantiate once with the Core base URL, then provide it via provideCoreClient().
 * All HTTP methods delegate to api.ts; WebSocket connections are created via openConsole().
 *
 * Key methods: getInstance, listInstances, createInstance, deleteInstance,
 * start/stop/kill/restart, issueWsTicket, openConsole, getStats,
 * listMods/addMod/uploadModFile/deleteMod/toggleMod/updateMod/updateAllMods,
 * listLogs/readLog, listCrashReports/readCrashReport,
 * getProperties/patchProperties, listDirectory/downloadFile/deleteFileOrFolder/uploadFile,
 * listBackups/createBackup/renameBackup/deleteBackup/deleteManyBackups/lockBackup/restoreBackup,
 * getBackupSchedule/setBackupSchedule.
 */

import type {
	CoreInstance,
	CoreInstanceSummary,
	CoreCreateInstanceBody,
	CoreStats,
	CoreMod,
	CoreFsListing,
	CoreBackupsResponse,
	CoreBackup,
	CoreBackupSchedule,
	CoreBackupScheduleBody,
	UploadHandle,
} from './types'
import * as api from './api'
import { CoreWsConnection } from './ws'

export class CoreApiClient {
	constructor(public readonly baseUrl: string) {}

	// ── Instances ───────────────────────────────────────────────────────────

	listInstances(): Promise<CoreInstanceSummary[]> {
		return api.listInstances(this.baseUrl).then((r) => r.instances)
	}

	getInstance(id: string): Promise<CoreInstance> {
		return api.getInstance(this.baseUrl, id)
	}

	createInstance(body: CoreCreateInstanceBody): Promise<CoreInstance> {
		return api.createInstance(this.baseUrl, body)
	}

	deleteInstance(id: string): Promise<void> {
		return api.deleteInstance(this.baseUrl, id).then(() => undefined)
	}

	// ── Lifecycle ───────────────────────────────────────────────────────────

	start(id: string): Promise<void> {
		return api.startInstance(this.baseUrl, id).then(() => undefined)
	}

	stop(id: string): Promise<void> {
		return api.stopInstance(this.baseUrl, id).then(() => undefined)
	}

	kill(id: string): Promise<void> {
		return api.killInstance(this.baseUrl, id).then(() => undefined)
	}

	restart(id: string): Promise<void> {
		return api.restartInstance(this.baseUrl, id).then(() => undefined)
	}

	sendCommand(id: string, command: string): Promise<void> {
		return api.sendCommand(this.baseUrl, id, command).then(() => undefined)
	}

	// ── WebSocket ───────────────────────────────────────────────────────────

	async issueWsTicket(): Promise<string> {
		return api.issueWsTicket(this.baseUrl).then((r) => r.ticket)
	}

	/**
	 * Opens a structured WebSocket console connection for an instance.
	 * Caller must first obtain a ticket via issueWsTicket().
	 */
	openConsole(instanceId: string, ticket: string): CoreWsConnection {
		const wsBase = this.baseUrl.replace(/^http/, 'ws')
		return new CoreWsConnection(`${wsBase}/instances/${instanceId}/console?ticket=${ticket}`)
	}

	// ── Stats ───────────────────────────────────────────────────────────────

	getStats(id: string): Promise<CoreStats> {
		return api.getStats(this.baseUrl, id)
	}

	// ── Mods ────────────────────────────────────────────────────────────────

	listMods(id: string): Promise<CoreMod[]> {
		return api.listMods(this.baseUrl, id).then((r) => r.mods)
	}

	addMod(id: string, versionId: string): Promise<CoreMod> {
		return api.addMod(this.baseUrl, id, versionId)
	}

	uploadModFile(id: string, file: File): UploadHandle {
		return api.uploadModFile(this.baseUrl, id, file)
	}

	deleteMod(id: string, filename: string): Promise<{ restart_required: boolean }> {
		return api.deleteMod(this.baseUrl, id, filename)
	}

	toggleMod(id: string, filename: string, enabled: boolean): Promise<void> {
		return api.toggleMod(this.baseUrl, id, filename, enabled).then(() => undefined)
	}

	updateMod(id: string, filename: string): Promise<boolean> {
		return api.updateMod(this.baseUrl, id, filename).then((r) => r.updated)
	}

	updateAllMods(
		id: string,
	): Promise<{ updated: string[]; already_latest: string[]; failed: unknown[] }> {
		return api.updateAllMods(this.baseUrl, id)
	}

	// ── Logs ────────────────────────────────────────────────────────────────

	listLogs(id: string): Promise<string[]> {
		return api.listLogs(this.baseUrl, id).then((r) => r.logs)
	}

	readLog(id: string, filename: string): Promise<string> {
		return api.readLog(this.baseUrl, id, filename)
	}

	listCrashReports(id: string): Promise<string[]> {
		return api.listCrashReports(this.baseUrl, id).then((r) => r.crash_reports)
	}

	readCrashReport(id: string, filename: string): Promise<string> {
		return api.readCrashReport(this.baseUrl, id, filename)
	}

	// ── Server properties ───────────────────────────────────────────────────

	getProperties(id: string): Promise<Record<string, string>> {
		return api.getProperties(this.baseUrl, id).then((r) => r.properties)
	}

	patchProperties(id: string, updates: Record<string, string>): Promise<void> {
		return api.patchProperties(this.baseUrl, id, updates).then(() => undefined)
	}

	// ── Filesystem ──────────────────────────────────────────────────────────

	listDirectory(id: string, path: string, page = 0, pageSize = 50): Promise<CoreFsListing> {
		return api.listDirectory(this.baseUrl, id, path, page, pageSize)
	}

	downloadFile(id: string, path: string): Promise<Blob> {
		return api.downloadFile(this.baseUrl, id, path)
	}

	deleteFileOrFolder(id: string, path: string, recursive = false): Promise<void> {
		return api.deleteFileOrFolder(this.baseUrl, id, path, recursive).then(() => undefined)
	}

	uploadFile(id: string, targetDir: string, file: File): UploadHandle {
		return api.uploadFile(this.baseUrl, id, targetDir, file)
	}

	// ── Backups ─────────────────────────────────────────────────────────────

	listBackups(id: string): Promise<CoreBackupsResponse> {
		return api.listBackups(this.baseUrl, id)
	}

	createBackup(id: string, name?: string): Promise<CoreBackup> {
		return api.createBackup(this.baseUrl, id, name)
	}

	renameBackup(id: string, backupId: string, name: string): Promise<void> {
		return api.renameBackup(this.baseUrl, id, backupId, name).then(() => undefined)
	}

	deleteBackup(id: string, backupId: string): Promise<void> {
		return api.deleteBackup(this.baseUrl, id, backupId).then(() => undefined)
	}

	deleteManyBackups(id: string, backupIds: string[]): Promise<number> {
		return api.deleteManyBackups(this.baseUrl, id, backupIds).then((r) => r.deleted)
	}

	lockBackup(id: string, backupId: string, locked: boolean): Promise<void> {
		return api.lockBackup(this.baseUrl, id, backupId, locked).then(() => undefined)
	}

	restoreBackup(id: string, backupId: string): Promise<void> {
		return api.restoreBackup(this.baseUrl, id, backupId).then(() => undefined)
	}

	getBackupSchedule(id: string): Promise<CoreBackupSchedule> {
		return api.getBackupSchedule(this.baseUrl, id)
	}

	setBackupSchedule(id: string, schedule: CoreBackupScheduleBody): Promise<void> {
		return api.setBackupSchedule(this.baseUrl, id, schedule).then(() => undefined)
	}
}
