/**
 * Raw HTTP fetch functions for every Amberite Core endpoint.
 * Each function accepts a `CoreCallContext` and returns a typed promise.
 *
 * Key functions: get/list/create/delete/patch instances, start/stop/kill/restart,
 * issueWsTicket, listMods/addMod/uploadMod/deleteMod/toggleMod/updateMod/updateAllMods,
 * getStats, listLogs/readLog, listCrashReports/readCrashReport,
 * getProperties/patchProperties, listFs/downloadFile/deleteFs/uploadFile,
 * listBackups/createBackup/deleteBackup/deleteManyBackups/lockBackup/restoreBackup,
 * getBackupSchedule/setBackupSchedule.
 */

import type { CoreCallContext } from './context'
import type {
	CoreInstance,
	CoreInstanceSummary,
	CoreCreateInstanceBody,
	CorePatchInstanceBody,
	CoreStats,
	CoreMod,
	CoreFsListing,
	CoreBackup,
	CoreBackupsResponse,
	CoreBackupSchedule,
	CoreBackupScheduleBody,
	UploadHandle,
	CoreFsEntry,
	FsDownloadUrlResponse,
	UnzipOption,
	FsZipRequest,
	FsCopyRequest,
} from './types'
import { NetworkError, CoreApiError } from './errors'

function authHeaders(ctx: CoreCallContext): Record<string, string> {
	const h: Record<string, string> = {}
	if (ctx.token) h['Authorization'] = `Bearer ${ctx.token}`
	return h
}

async function apiFetch<T>(ctx: CoreCallContext, url: string, init?: RequestInit): Promise<T> {
	let res: Response
	try {
		res = await ctx.fetchFn(url, {
			...init,
			headers: { ...authHeaders(ctx), ...(init?.headers || {}) },
		})
	} catch (e) {
		const reason = e instanceof Error ? e.message : String(e)
		throw new NetworkError(reason)
	}
	if (!res.ok) {
		let msg = res.statusText
		try {
			const body = await res.json()
			if (body?.error) msg = body.error
		} catch {
			// ignore parse error — use statusText
		}
		throw new CoreApiError(res.status, msg)
	}
	try {
		return await res.json()
	} catch (e) {
		const reason = e instanceof Error ? e.message : String(e)
		throw new NetworkError(`Invalid JSON response from ${url} — ${reason}`)
	}
}

// ── Instances ─────────────────────────────────────────────────────────────────

export function listInstances(ctx: CoreCallContext): Promise<{ instances: CoreInstanceSummary[] }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances`)
}

export function getInstance(ctx: CoreCallContext, id: string): Promise<CoreInstance> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}`)
}

export function createInstance(
	ctx: CoreCallContext,
	body: CoreCreateInstanceBody,
): Promise<CoreInstance> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function deleteInstance(ctx: CoreCallContext, id: string): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}`, { method: 'DELETE' })
}

export function patchInstance(
	ctx: CoreCallContext,
	id: string,
	body: CorePatchInstanceBody,
): Promise<CoreInstance> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

// ── Instance lifecycle ────────────────────────────────────────────────────────

export function startInstance(ctx: CoreCallContext, id: string): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/start`, { method: 'POST' })
}

export function stopInstance(ctx: CoreCallContext, id: string): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/stop`, { method: 'POST' })
}

export function killInstance(ctx: CoreCallContext, id: string): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/kill`, { method: 'POST' })
}

export function restartInstance(ctx: CoreCallContext, id: string): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/restart`, { method: 'POST' })
}

export function sendCommand(
	ctx: CoreCallContext,
	id: string,
	command: string,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/command`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ command }),
	})
}

// ── WebSocket ticket ──────────────────────────────────────────────────────────

export function issueWsTicket(ctx: CoreCallContext): Promise<{ ticket: string }> {
	return apiFetch(ctx, `${ctx.baseUrl}/ws-token`, { method: 'POST' })
}

// ── Stats ─────────────────────────────────────────────────────────────────────

export function getStats(ctx: CoreCallContext, id: string): Promise<CoreStats> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/stats`)
}

// ── Mods ──────────────────────────────────────────────────────────────────────

export function listMods(ctx: CoreCallContext, id: string): Promise<{ mods: CoreMod[] }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/mods`)
}

export function addMod(ctx: CoreCallContext, id: string, versionId: string): Promise<CoreMod> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/mods`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ version_id: versionId }),
	})
}

export function uploadModFile(ctx: CoreCallContext, id: string, file: File): UploadHandle {
	return xhrUpload(ctx, `${ctx.baseUrl}/instances/${id}/mods/upload`, file)
}

export function deleteMod(
	ctx: CoreCallContext,
	id: string,
	filename: string,
): Promise<{ ok: boolean; restart_required: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/mods/${encodeURIComponent(filename)}`, {
		method: 'DELETE',
	})
}

export function toggleMod(
	ctx: CoreCallContext,
	id: string,
	filename: string,
	enabled: boolean,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/mods/${encodeURIComponent(filename)}`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ enabled }),
	})
}

export function updateMod(
	ctx: CoreCallContext,
	id: string,
	filename: string,
): Promise<{ updated: boolean }> {
	return apiFetch(
		ctx,
		`${ctx.baseUrl}/instances/${id}/mods/${encodeURIComponent(filename)}/update`,
		{
			method: 'PUT',
		},
	)
}

export function updateAllMods(
	ctx: CoreCallContext,
	id: string,
): Promise<{ updated: string[]; already_latest: string[]; failed: unknown[] }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/mods/update-all`, { method: 'POST' })
}

// ── Logs ──────────────────────────────────────────────────────────────────────

export function listLogs(ctx: CoreCallContext, id: string): Promise<{ logs: string[] }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/logs`)
}

export function readLog(ctx: CoreCallContext, id: string, filename: string): Promise<string> {
	return ctx
		.fetchFn(`${ctx.baseUrl}/instances/${id}/logs/${encodeURIComponent(filename)}`, {
			headers: authHeaders(ctx),
		})
		.then((r) => {
			if (!r.ok) throw new CoreApiError(r.status, r.statusText)
			return r.text()
		})
}

export function listCrashReports(
	ctx: CoreCallContext,
	id: string,
): Promise<{ crash_reports: string[] }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/crash-reports`)
}

export function readCrashReport(
	ctx: CoreCallContext,
	id: string,
	filename: string,
): Promise<string> {
	return ctx
		.fetchFn(`${ctx.baseUrl}/instances/${id}/crash-reports/${encodeURIComponent(filename)}`, {
			headers: authHeaders(ctx),
		})
		.then((r) => {
			if (!r.ok) throw new CoreApiError(r.status, r.statusText)
			return r.text()
		})
}

// ── Server properties ─────────────────────────────────────────────────────────

export function getProperties(
	ctx: CoreCallContext,
	id: string,
): Promise<{ properties: Record<string, string> }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/properties`)
}

export function patchProperties(
	ctx: CoreCallContext,
	id: string,
	updates: Record<string, string>,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/properties`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(updates),
	})
}

// ── Filesystem ────────────────────────────────────────────────────────────────

export function listDirectory(
	ctx: CoreCallContext,
	id: string,
	path: string,
	page = 0,
	pageSize = 50,
): Promise<CoreFsListing> {
	const q = new URLSearchParams({
		path,
		page: String(page),
		page_size: String(pageSize),
	})
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/fs?${q}`)
}

export function downloadFile(ctx: CoreCallContext, id: string, path: string): Promise<Blob> {
	const q = new URLSearchParams({ path })
	return ctx
		.fetchFn(`${ctx.baseUrl}/instances/${id}/fs/download?${q}`, {
			headers: authHeaders(ctx),
		})
		.then((r) => {
			if (!r.ok) throw new CoreApiError(r.status, r.statusText)
			return r.blob()
		})
}

export function deleteFileOrFolder(
	ctx: CoreCallContext,
	id: string,
	path: string,
	recursive = false,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/fs`, {
		method: 'DELETE',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ path, recursive }),
	})
}

export function uploadFile(
	ctx: CoreCallContext,
	id: string,
	targetDir: string,
	file: File,
): UploadHandle {
	const url = `${ctx.baseUrl}/instances/${id}/fs/upload?path=${encodeURIComponent(targetDir)}`
	return xhrUpload(ctx, url, file)
}

export function readFile(ctx: CoreCallContext, id: string, path: string): Promise<ArrayBuffer> {
	const q = new URLSearchParams({ path })
	return ctx
		.fetchFn(`${ctx.baseUrl}/instances/${id}/fs/read?${q}`, { headers: authHeaders(ctx) })
		.then((r) => {
			if (!r.ok) throw new CoreApiError(r.status, r.statusText)
			return r.arrayBuffer()
		})
}

export function writeFile(
	ctx: CoreCallContext,
	id: string,
	path: string,
	content: string | ArrayBuffer,
): Promise<{ ok: boolean }> {
	const q = new URLSearchParams({ path })
	const body = typeof content === 'string' ? new TextEncoder().encode(content) : content
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/fs/write?${q}`, {
		method: 'PUT',
		body,
	})
}

export function createFile(
	ctx: CoreCallContext,
	id: string,
	path: string,
): Promise<{ ok: boolean }> {
	const q = new URLSearchParams({ path })
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/fs/create?${q}`, { method: 'POST' })
}

export function createDir(
	ctx: CoreCallContext,
	id: string,
	path: string,
): Promise<{ ok: boolean }> {
	const q = new URLSearchParams({ path })
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/fs/mkdir?${q}`, { method: 'POST' })
}

export function moveEntry(
	ctx: CoreCallContext,
	id: string,
	from: string,
	to: string,
): Promise<{ ok: boolean }> {
	const q = new URLSearchParams({ from, to })
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/fs/move?${q}`, { method: 'PUT' })
}

export function unzipFile(
	ctx: CoreCallContext,
	id: string,
	path: string,
	option: UnzipOption,
): Promise<{ ok: boolean }> {
	const q = new URLSearchParams({ path })
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/fs/unzip?${q}`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ option }),
	})
}

export function zipFiles(
	ctx: CoreCallContext,
	id: string,
	req: FsZipRequest,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/fs/zip`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(req),
	})
}

export function copyFiles(
	ctx: CoreCallContext,
	id: string,
	req: FsCopyRequest,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/fs/copy`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(req),
	})
}

export function getDownloadUrl(
	ctx: CoreCallContext,
	id: string,
	path: string,
): Promise<FsDownloadUrlResponse> {
	const q = new URLSearchParams({ path })
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/fs/url?${q}`)
}

export function searchFiles(
	ctx: CoreCallContext,
	id: string,
	path: string,
	query: string,
	recursive: boolean,
): Promise<CoreFsEntry[]> {
	const q = new URLSearchParams({ path, query, recursive: String(recursive) })
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/fs/search?${q}`)
}

// ── Backups ───────────────────────────────────────────────────────────────────

export function listBackups(ctx: CoreCallContext, id: string): Promise<CoreBackupsResponse> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/backups`)
}

export function createBackup(ctx: CoreCallContext, id: string, name?: string): Promise<CoreBackup> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/backups`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ name: name ?? null }),
	})
}

export function deleteBackup(
	ctx: CoreCallContext,
	id: string,
	backupId: string,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/backups/${backupId}`, { method: 'DELETE' })
}

export function deleteManyBackups(
	ctx: CoreCallContext,
	id: string,
	backupIds: string[],
): Promise<{ deleted: number }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/backups/delete-many`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ ids: backupIds }),
	})
}

export function renameBackup(
	ctx: CoreCallContext,
	id: string,
	backupId: string,
	name: string,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/backups/${backupId}`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ name }),
	})
}

export function lockBackup(
	ctx: CoreCallContext,
	id: string,
	backupId: string,
	locked: boolean,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/backups/${backupId}/lock`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ locked }),
	})
}

export function restoreBackup(
	ctx: CoreCallContext,
	id: string,
	backupId: string,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/backups/${backupId}/restore`, {
		method: 'POST',
	})
}

export function getBackupSchedule(ctx: CoreCallContext, id: string): Promise<CoreBackupSchedule> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/backups/schedule`)
}

export function setBackupSchedule(
	ctx: CoreCallContext,
	id: string,
	schedule: CoreBackupScheduleBody,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances/${id}/backups/schedule`, {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(schedule),
	})
}

// ── Internal: XHR upload ──────────────────────────────────────────────────────

function xhrUpload(ctx: CoreCallContext, url: string, file: File): UploadHandle {
	const progressCallbacks: Array<(percent: number) => void> = []
	let resolve!: () => void
	let reject!: (e: Error) => void
	const done = new Promise<void>((res, rej) => {
		resolve = res
		reject = rej
	})

	const xhr = new XMLHttpRequest()
	xhr.open('POST', url)
	if (ctx.token) xhr.setRequestHeader('Authorization', `Bearer ${ctx.token}`)
	xhr.upload.onprogress = (e) => {
		if (e.lengthComputable) {
			const pct = Math.round((e.loaded / e.total) * 100)
			progressCallbacks.forEach((cb) => cb(pct))
		}
	}
	xhr.onload = () => {
		if (xhr.status >= 200 && xhr.status < 300) resolve()
		else reject(new CoreApiError(xhr.status, 'upload failed'))
	}
	xhr.onerror = () => reject(new NetworkError('Network error during upload'))

	const form = new FormData()
	form.append('file', file, file.name)
	xhr.send(form)

	return {
		onProgress: (cb) => progressCallbacks.push(cb),
		done,
		abort: () => xhr.abort(),
	}
}
