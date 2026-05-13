/**
 * CoreApiClient — primary entry point for all Amberite Core API calls.
 *
 * Accepts a PlatformAdapter, attempts direct HTTP first, and falls through to
 * the Supabase relay when direct HTTP is unreachable. From the caller's
 * perspective every method works the same regardless of transport.
 *
 * Key methods: getInstance, listInstances, createInstance, deleteInstance, renameInstance,
 * updateJavaVersion, start/stop/kill/restart, issueWsTicket, openConsole, getStats,
 * listMods/addMod/uploadModFile/deleteMod/toggleMod/updateMod/updateAllMods,
 * listLogs/readLog, listCrashReports/readCrashReport,
 * getProperties/patchProperties, listDirectory/downloadFile/deleteFileOrFolder/uploadFile,
 * listBackups/createBackup/renameBackup/deleteBackup/deleteManyBackups/lockBackup/restoreBackup,
 * getBackupSchedule/setBackupSchedule.
 */

import type { PlatformAdapter } from './adapter'
import type { CoreCallContext } from './context'
import type {
	CoreInstance,
	CoreInstanceSummary,
	CoreCreateInstanceBody,
	CorePatchInstanceBody,
	CoreStats,
	CoreMod,
	CoreFsListing,
	CoreBackupsResponse,
	CoreBackup,
	CoreBackupSchedule,
	CoreBackupScheduleBody,
	UploadHandle,
	FsDownloadUrlResponse,
	UnzipOption,
	FsZipRequest,
	FsCopyRequest,
} from './types'
import * as api from './api'
import { CoreWsConnection } from './ws'
import { publishMessage, waitForReceipt, waitForResult } from './transport'
import { NetworkError, CoreOfflineError } from './errors'
import type { CoreConnectionMonitor } from './monitor'

interface RelayApiPayload {
	method: string
	path: string
	body?: unknown
}

export class CoreApiClient {
	public monitor: CoreConnectionMonitor | null = null
	private coreUrlPromise: Promise<string | null> | null = null

	constructor(public readonly adapter: PlatformAdapter) {}

	private getCoreUrlCached(): Promise<string | null> {
		if (!this.coreUrlPromise) {
			this.coreUrlPromise = this.adapter.getCoreUrl().then((url) => {
				// Don't cache a null result — Core may come online later.
				if (url === null) this.coreUrlPromise = null
				return url
			})
		}
		return this.coreUrlPromise
	}

	private async direct<T>(fn: (ctx: CoreCallContext) => Promise<T>): Promise<T> {
		const coreUrl = await this.getCoreUrlCached()
		const token = await this.adapter.getLocalCoreToken()
		if (!coreUrl) throw new CoreOfflineError()
		const ctx: CoreCallContext = {
			baseUrl: coreUrl,
			token,
			fetchFn: this.adapter.fetchFn,
		}
		return fn(ctx)
	}

	private async relay<T>(payload: RelayApiPayload): Promise<T> {
		const coreUrl = await this.getCoreUrlCached()
		if (!coreUrl) throw new CoreOfflineError()

		const coreId = new URL(coreUrl).hostname // simplistic core-id derivation; caller may override
		const senderId = (await this.adapter.getCurrentJwt()) ?? 'anonymous'

		let msg: Awaited<ReturnType<typeof publishMessage>>
		try {
			msg = await publishMessage(this.adapter.supabase, {
				coreId,
				senderId,
				direction: 'client-to-core',
				payload,
			})
		} catch {
			throw new NetworkError('Core server is offline and the relay is unavailable.')
		}

		await waitForReceipt(this.adapter.supabase, msg.id)
		const result = await waitForResult(this.adapter.supabase, msg.id)

		if (result && typeof result === 'object' && 'error' in result) {
			throw new NetworkError(String((result as any).error))
		}
		return result as T
	}

	private async request<T>(
		directFn: (ctx: CoreCallContext) => Promise<T>,
		relayPayload: RelayApiPayload,
	): Promise<T> {
		try {
			return await this.direct(directFn)
		} catch (e) {
			if (e instanceof NetworkError || e instanceof CoreOfflineError) {
				return this.relay<T>(relayPayload)
			}
			throw e
		}
	}

	// ── Instances ───────────────────────────────────────────────────────────

	listInstances(): Promise<CoreInstanceSummary[]> {
		return this.request((ctx) => api.listInstances(ctx).then((r) => r.instances), {
			method: 'GET',
			path: '/instances',
		})
	}

	getInstance(id: string): Promise<CoreInstance> {
		return this.request((ctx) => api.getInstance(ctx, id), {
			method: 'GET',
			path: `/instances/${id}`,
		})
	}

	createInstance(body: CoreCreateInstanceBody): Promise<CoreInstance> {
		return this.request((ctx) => api.createInstance(ctx, body), {
			method: 'POST',
			path: '/instances',
			body,
		})
	}

	deleteInstance(id: string): Promise<void> {
		return this.request((ctx) => api.deleteInstance(ctx, id).then(() => undefined), {
			method: 'DELETE',
			path: `/instances/${id}`,
		})
	}

	renameInstance(id: string, name: string): Promise<CoreInstance> {
		return this.request((ctx) => api.patchInstance(ctx, id, { name }), {
			method: 'PATCH',
			path: `/instances/${id}`,
			body: { name },
		})
	}

	updateJavaVersion(id: string, javaVersion: number | null): Promise<CoreInstance> {
		return this.request((ctx) => api.patchInstance(ctx, id, { java_version: javaVersion }), {
			method: 'PATCH',
			path: `/instances/${id}`,
			body: { java_version: javaVersion },
		})
	}

	// ── Lifecycle ───────────────────────────────────────────────────────────

	start(id: string): Promise<void> {
		return this.request((ctx) => api.startInstance(ctx, id).then(() => undefined), {
			method: 'POST',
			path: `/instances/${id}/start`,
		})
	}

	stop(id: string): Promise<void> {
		return this.request((ctx) => api.stopInstance(ctx, id).then(() => undefined), {
			method: 'POST',
			path: `/instances/${id}/stop`,
		})
	}

	kill(id: string): Promise<void> {
		return this.request((ctx) => api.killInstance(ctx, id).then(() => undefined), {
			method: 'POST',
			path: `/instances/${id}/kill`,
		})
	}

	restart(id: string): Promise<void> {
		return this.request((ctx) => api.restartInstance(ctx, id).then(() => undefined), {
			method: 'POST',
			path: `/instances/${id}/restart`,
		})
	}

	sendCommand(id: string, command: string): Promise<void> {
		return this.request((ctx) => api.sendCommand(ctx, id, command).then(() => undefined), {
			method: 'POST',
			path: `/instances/${id}/command`,
			body: { command },
		})
	}

	// ── WebSocket ───────────────────────────────────────────────────────────

	async issueWsTicket(): Promise<string> {
		return this.request((ctx) => api.issueWsTicket(ctx).then((r) => r.ticket), {
			method: 'POST',
			path: '/ws-token',
		})
	}

	async openConsole(instanceId: string, ticket: string): Promise<CoreWsConnection> {
		const coreUrl = await this.getCoreUrlCached()
		if (!coreUrl) throw new CoreOfflineError()
		const wsUrl =
			coreUrl.replace(/^http/, 'ws') +
			`/instances/${encodeURIComponent(instanceId)}/console?ticket=${encodeURIComponent(ticket)}`
		return new CoreWsConnection(wsUrl)
	}

	// ── Stats ───────────────────────────────────────────────────────────────

	getStats(id: string): Promise<CoreStats> {
		return this.request((ctx) => api.getStats(ctx, id), {
			method: 'GET',
			path: `/instances/${id}/stats`,
		})
	}

	// ── Mods ────────────────────────────────────────────────────────────────

	listMods(id: string): Promise<CoreMod[]> {
		return this.request((ctx) => api.listMods(ctx, id).then((r) => r.mods), {
			method: 'GET',
			path: `/instances/${id}/mods`,
		})
	}

	addMod(id: string, versionId: string): Promise<CoreMod> {
		return this.request((ctx) => api.addMod(ctx, id, versionId), {
			method: 'POST',
			path: `/instances/${id}/mods`,
			body: { version_id: versionId },
		})
	}

	uploadModFile(id: string, file: File): UploadHandle {
		const progressCallbacks: Array<(percent: number) => void> = []
		let innerAbort = () => {}
		let aborted = false
		const done = (async () => {
			const coreUrl = await this.getCoreUrlCached()
			const token = await this.adapter.getLocalCoreToken()
			if (!coreUrl) throw new CoreOfflineError()
			if (aborted) return
			const ctx: CoreCallContext = { baseUrl: coreUrl, token, fetchFn: this.adapter.fetchFn }
			const handle = api.uploadModFile(ctx, id, file)
			innerAbort = handle.abort
			if (aborted) {
				handle.abort()
				return
			}
			handle.onProgress((pct) => progressCallbacks.forEach((cb) => cb(pct)))
			await handle.done
		})()
		return {
			onProgress: (cb) => progressCallbacks.push(cb),
			done,
			abort: () => {
				aborted = true
				innerAbort()
			},
		}
	}

	deleteMod(id: string, filename: string): Promise<{ restart_required: boolean }> {
		return this.request((ctx) => api.deleteMod(ctx, id, filename), {
			method: 'DELETE',
			path: `/instances/${id}/mods/${encodeURIComponent(filename)}`,
		})
	}

	toggleMod(id: string, filename: string, enabled: boolean): Promise<void> {
		return this.request((ctx) => api.toggleMod(ctx, id, filename, enabled).then(() => undefined), {
			method: 'PATCH',
			path: `/instances/${id}/mods/${encodeURIComponent(filename)}`,
			body: { enabled },
		})
	}

	updateMod(id: string, filename: string): Promise<boolean> {
		return this.request((ctx) => api.updateMod(ctx, id, filename).then((r) => r.updated), {
			method: 'PUT',
			path: `/instances/${id}/mods/${encodeURIComponent(filename)}/update`,
		})
	}

	updateAllMods(
		id: string,
	): Promise<{ updated: string[]; already_latest: string[]; failed: unknown[] }> {
		return this.request((ctx) => api.updateAllMods(ctx, id), {
			method: 'POST',
			path: `/instances/${id}/mods/update-all`,
		})
	}

	// ── Logs ────────────────────────────────────────────────────────────────

	listLogs(id: string): Promise<string[]> {
		return this.request((ctx) => api.listLogs(ctx, id).then((r) => r.logs), {
			method: 'GET',
			path: `/instances/${id}/logs`,
		})
	}

	readLog(id: string, filename: string): Promise<string> {
		return this.request((ctx) => api.readLog(ctx, id, filename), {
			method: 'GET',
			path: `/instances/${id}/logs/${encodeURIComponent(filename)}`,
		})
	}

	listCrashReports(id: string): Promise<string[]> {
		return this.request((ctx) => api.listCrashReports(ctx, id).then((r) => r.crash_reports), {
			method: 'GET',
			path: `/instances/${id}/crash-reports`,
		})
	}

	readCrashReport(id: string, filename: string): Promise<string> {
		return this.request((ctx) => api.readCrashReport(ctx, id, filename), {
			method: 'GET',
			path: `/instances/${id}/crash-reports/${encodeURIComponent(filename)}`,
		})
	}

	// ── Server properties ───────────────────────────────────────────────────

	getProperties(id: string): Promise<Record<string, string>> {
		return this.request((ctx) => api.getProperties(ctx, id).then((r) => r.properties), {
			method: 'GET',
			path: `/instances/${id}/properties`,
		})
	}

	patchProperties(id: string, updates: Record<string, string>): Promise<void> {
		return this.request((ctx) => api.patchProperties(ctx, id, updates).then(() => undefined), {
			method: 'PATCH',
			path: `/instances/${id}/properties`,
			body: updates,
		})
	}

	// ── Filesystem ──────────────────────────────────────────────────────────

	listDirectory(id: string, path: string, page = 0, pageSize = 50): Promise<CoreFsListing> {
		return this.request((ctx) => api.listDirectory(ctx, id, path, page, pageSize), {
			method: 'GET',
			path: `/instances/${id}/fs?page=${page}&page_size=${pageSize}&path=${encodeURIComponent(path)}`,
		})
	}

	downloadFile(id: string, path: string): Promise<Blob> {
		return this.request((ctx) => api.downloadFile(ctx, id, path), {
			method: 'GET',
			path: `/instances/${id}/fs/download?path=${encodeURIComponent(path)}`,
		})
	}

	deleteFileOrFolder(id: string, path: string, recursive = false): Promise<void> {
		return this.request(
			(ctx) => api.deleteFileOrFolder(ctx, id, path, recursive).then(() => undefined),
			{ method: 'DELETE', path: `/instances/${id}/fs`, body: { path, recursive } },
		)
	}

	uploadFile(id: string, targetDir: string, file: File): UploadHandle {
		const progressCallbacks: Array<(percent: number) => void> = []
		let innerAbort = () => {}
		let aborted = false
		const done = (async () => {
			const coreUrl = await this.getCoreUrlCached()
			const token = await this.adapter.getLocalCoreToken()
			if (!coreUrl) throw new CoreOfflineError()
			if (aborted) return
			const ctx: CoreCallContext = { baseUrl: coreUrl, token, fetchFn: this.adapter.fetchFn }
			const handle = api.uploadFile(ctx, id, targetDir, file)
			innerAbort = handle.abort
			if (aborted) {
				handle.abort()
				return
			}
			handle.onProgress((pct) => progressCallbacks.forEach((cb) => cb(pct)))
			await handle.done
		})()
		return {
			onProgress: (cb) => progressCallbacks.push(cb),
			done,
			abort: () => {
				aborted = true
				innerAbort()
			},
		}
	}

	readFile(id: string, path: string): Promise<ArrayBuffer> {
		return this.request((ctx) => api.readFile(ctx, id, path), {
			method: 'GET',
			path: `/instances/${id}/fs/read?path=${encodeURIComponent(path)}`,
		})
	}

	writeFile(id: string, path: string, content: string | ArrayBuffer): Promise<void> {
		return this.request((ctx) => api.writeFile(ctx, id, path, content).then(() => undefined), {
			method: 'PUT',
			path: `/instances/${id}/fs/write?path=${encodeURIComponent(path)}`,
			body: content,
		})
	}

	createFile(id: string, path: string): Promise<void> {
		return this.request((ctx) => api.createFile(ctx, id, path).then(() => undefined), {
			method: 'POST',
			path: `/instances/${id}/fs/create?path=${encodeURIComponent(path)}`,
		})
	}

	createDir(id: string, path: string): Promise<void> {
		return this.request((ctx) => api.createDir(ctx, id, path).then(() => undefined), {
			method: 'POST',
			path: `/instances/${id}/fs/mkdir?path=${encodeURIComponent(path)}`,
		})
	}

	moveEntry(id: string, from: string, to: string): Promise<void> {
		return this.request((ctx) => api.moveEntry(ctx, id, from, to).then(() => undefined), {
			method: 'PUT',
			path: `/instances/${id}/fs/move?from=${encodeURIComponent(from)}&to=${encodeURIComponent(to)}`,
		})
	}

	unzipFile(id: string, path: string, option: UnzipOption = 'normal'): Promise<void> {
		return this.request((ctx) => api.unzipFile(ctx, id, path, option).then(() => undefined), {
			method: 'POST',
			path: `/instances/${id}/fs/unzip?path=${encodeURIComponent(path)}`,
			body: { option },
		})
	}

	zipFiles(id: string, req: FsZipRequest): Promise<void> {
		return this.request((ctx) => api.zipFiles(ctx, id, req).then(() => undefined), {
			method: 'POST',
			path: `/instances/${id}/fs/zip`,
			body: req,
		})
	}

	copyFiles(id: string, req: FsCopyRequest): Promise<void> {
		return this.request((ctx) => api.copyFiles(ctx, id, req).then(() => undefined), {
			method: 'POST',
			path: `/instances/${id}/fs/copy`,
			body: req,
		})
	}

	getDownloadUrl(id: string, path: string): Promise<FsDownloadUrlResponse> {
		return this.request((ctx) => api.getDownloadUrl(ctx, id, path), {
			method: 'GET',
			path: `/instances/${id}/fs/url?path=${encodeURIComponent(path)}`,
		})
	}

	searchFiles(
		id: string,
		path: string,
		query: string,
		recursive = false,
	): Promise<import('./types').CoreFsEntry[]> {
		return this.request((ctx) => api.searchFiles(ctx, id, path, query, recursive), {
			method: 'GET',
			path: `/instances/${id}/fs/search?path=${encodeURIComponent(path)}&query=${encodeURIComponent(query)}&recursive=${recursive}`,
		})
	}

	// ── Backups ─────────────────────────────────────────────────────────────

	listBackups(id: string): Promise<CoreBackupsResponse> {
		return this.request((ctx) => api.listBackups(ctx, id), {
			method: 'GET',
			path: `/instances/${id}/backups`,
		})
	}

	createBackup(id: string, name?: string): Promise<CoreBackup> {
		return this.request((ctx) => api.createBackup(ctx, id, name), {
			method: 'POST',
			path: `/instances/${id}/backups`,
			body: { name: name ?? null },
		})
	}

	renameBackup(id: string, backupId: string, name: string): Promise<void> {
		return this.request((ctx) => api.renameBackup(ctx, id, backupId, name).then(() => undefined), {
			method: 'PATCH',
			path: `/instances/${id}/backups/${backupId}`,
			body: { name },
		})
	}

	deleteBackup(id: string, backupId: string): Promise<void> {
		return this.request((ctx) => api.deleteBackup(ctx, id, backupId).then(() => undefined), {
			method: 'DELETE',
			path: `/instances/${id}/backups/${backupId}`,
		})
	}

	deleteManyBackups(id: string, backupIds: string[]): Promise<number> {
		return this.request((ctx) => api.deleteManyBackups(ctx, id, backupIds).then((r) => r.deleted), {
			method: 'POST',
			path: `/instances/${id}/backups/delete-many`,
			body: { ids: backupIds },
		})
	}

	lockBackup(id: string, backupId: string, locked: boolean): Promise<void> {
		return this.request((ctx) => api.lockBackup(ctx, id, backupId, locked).then(() => undefined), {
			method: 'PATCH',
			path: `/instances/${id}/backups/${backupId}/lock`,
			body: { locked },
		})
	}

	restoreBackup(id: string, backupId: string): Promise<void> {
		return this.request((ctx) => api.restoreBackup(ctx, id, backupId).then(() => undefined), {
			method: 'POST',
			path: `/instances/${id}/backups/${backupId}/restore`,
		})
	}

	getBackupSchedule(id: string): Promise<CoreBackupSchedule> {
		return this.request((ctx) => api.getBackupSchedule(ctx, id), {
			method: 'GET',
			path: `/instances/${id}/backups/schedule`,
		})
	}

	setBackupSchedule(id: string, schedule: CoreBackupScheduleBody): Promise<void> {
		return this.request((ctx) => api.setBackupSchedule(ctx, id, schedule).then(() => undefined), {
			method: 'PUT',
			path: `/instances/${id}/backups/schedule`,
			body: schedule,
		})
	}
}
