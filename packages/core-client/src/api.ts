/**
 * Raw HTTP fetch functions for every Amberite Core endpoint.
 * Each function accepts a `baseUrl` (no trailing slash) and returns a typed promise.
 * No Tauri-specific code — uses the native fetch API.
 *
 * Key functions: get/list/create/delete instances, start/stop/kill/restart,
 * issueWsTicket, listMods/addMod/uploadMod/deleteMod/toggleMod/updateMod/updateAllMods,
 * getStats, listLogs/readLog, listCrashReports/readCrashReport,
 * getProperties/patchProperties, listFs/downloadFile/deleteFs/uploadFile,
 * listBackups/createBackup/deleteBackup/deleteManyBackups/lockBackup/restoreBackup,
 * getBackupSchedule/setBackupSchedule.
 */

import type {
	CoreInstance,
	CoreInstanceSummary,
	CoreCreateInstanceBody,
	CoreStats,
	CoreMod,
	CoreFsListing,
	CoreBackup,
	CoreBackupsResponse,
	CoreBackupSchedule,
	CoreBackupScheduleBody,
	UploadHandle,
} from './types'

async function apiFetch<T>(url: string, init?: RequestInit): Promise<T> {
	const res = await fetch(url, init)
	if (!res.ok) {
		let msg = res.statusText
		try {
			const body = await res.json()
			if (body?.error) msg = body.error
		} catch {
			// ignore parse error — use statusText
		}
		throw new Error(`Core API ${res.status}: ${msg}`)
	}
	return res.json() as Promise<T>
}

// ── Instances ─────────────────────────────────────────────────────────────────

export function listInstances(base: string): Promise<{ instances: CoreInstanceSummary[] }> {
	return apiFetch(`${base}/instances`)
}

export function getInstance(base: string, id: string): Promise<CoreInstance> {
	return apiFetch(`${base}/instances/${id}`)
}

export function createInstance(base: string, body: CoreCreateInstanceBody): Promise<CoreInstance> {
	return apiFetch(`${base}/instances`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function deleteInstance(base: string, id: string): Promise<{ ok: boolean }> {
	return apiFetch(`${base}/instances/${id}`, { method: 'DELETE' })
}

// ── Instance lifecycle ────────────────────────────────────────────────────────

export function startInstance(base: string, id: string): Promise<{ ok: boolean }> {
	return apiFetch(`${base}/instances/${id}/start`, { method: 'POST' })
}

export function stopInstance(base: string, id: string): Promise<{ ok: boolean }> {
	return apiFetch(`${base}/instances/${id}/stop`, { method: 'POST' })
}

export function killInstance(base: string, id: string): Promise<{ ok: boolean }> {
	return apiFetch(`${base}/instances/${id}/kill`, { method: 'POST' })
}

export function restartInstance(base: string, id: string): Promise<{ ok: boolean }> {
	return apiFetch(`${base}/instances/${id}/restart`, { method: 'POST' })
}

export function sendCommand(base: string, id: string, command: string): Promise<{ ok: boolean }> {
	return apiFetch(`${base}/instances/${id}/command`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ command }),
	})
}

// ── WebSocket ticket ──────────────────────────────────────────────────────────

export function issueWsTicket(base: string): Promise<{ ticket: string }> {
	return apiFetch(`${base}/ws-token`, { method: 'POST' })
}

// ── Stats ─────────────────────────────────────────────────────────────────────

export function getStats(base: string, id: string): Promise<CoreStats> {
	return apiFetch(`${base}/instances/${id}/stats`)
}

// ── Mods ──────────────────────────────────────────────────────────────────────

export function listMods(base: string, id: string): Promise<{ mods: CoreMod[] }> {
	return apiFetch(`${base}/instances/${id}/mods`)
}

export function addMod(base: string, id: string, versionId: string): Promise<CoreMod> {
	return apiFetch(`${base}/instances/${id}/mods`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ version_id: versionId }),
	})
}

export function uploadModFile(base: string, id: string, file: File): UploadHandle {
	return xhrUpload(`${base}/instances/${id}/mods/upload`, file)
}

export function deleteMod(
	base: string,
	id: string,
	filename: string,
): Promise<{ ok: boolean; restart_required: boolean }> {
	return apiFetch(`${base}/instances/${id}/mods/${encodeURIComponent(filename)}`, {
		method: 'DELETE',
	})
}

export function toggleMod(
	base: string,
	id: string,
	filename: string,
	enabled: boolean,
): Promise<{ ok: boolean }> {
	return apiFetch(`${base}/instances/${id}/mods/${encodeURIComponent(filename)}`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ enabled }),
	})
}

export function updateMod(
	base: string,
	id: string,
	filename: string,
): Promise<{ updated: boolean }> {
	return apiFetch(`${base}/instances/${id}/mods/${encodeURIComponent(filename)}/update`, {
		method: 'PUT',
	})
}

export function updateAllMods(
	base: string,
	id: string,
): Promise<{ updated: string[]; already_latest: string[]; failed: unknown[] }> {
	return apiFetch(`${base}/instances/${id}/mods/update-all`, { method: 'POST' })
}

// ── Logs ──────────────────────────────────────────────────────────────────────

export function listLogs(base: string, id: string): Promise<{ logs: string[] }> {
	return apiFetch(`${base}/instances/${id}/logs`)
}

export function readLog(base: string, id: string, filename: string): Promise<string> {
	return fetch(`${base}/instances/${id}/logs/${encodeURIComponent(filename)}`).then((r) => {
		if (!r.ok) throw new Error(`Core API ${r.status}: ${r.statusText}`)
		return r.text()
	})
}

export function listCrashReports(base: string, id: string): Promise<{ crash_reports: string[] }> {
	return apiFetch(`${base}/instances/${id}/crash-reports`)
}

export function readCrashReport(base: string, id: string, filename: string): Promise<string> {
	return fetch(`${base}/instances/${id}/crash-reports/${encodeURIComponent(filename)}`).then(
		(r) => {
			if (!r.ok) throw new Error(`Core API ${r.status}: ${r.statusText}`)
			return r.text()
		},
	)
}

// ── Server properties ─────────────────────────────────────────────────────────

export function getProperties(
	base: string,
	id: string,
): Promise<{ properties: Record<string, string> }> {
	return apiFetch(`${base}/instances/${id}/properties`)
}

export function patchProperties(
	base: string,
	id: string,
	updates: Record<string, string>,
): Promise<{ ok: boolean }> {
	return apiFetch(`${base}/instances/${id}/properties`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(updates),
	})
}

// ── Filesystem ────────────────────────────────────────────────────────────────

export function listDirectory(
	base: string,
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
	return apiFetch(`${base}/instances/${id}/fs?${q}`)
}

export function downloadFile(base: string, id: string, path: string): Promise<Blob> {
	const q = new URLSearchParams({ path })
	return fetch(`${base}/instances/${id}/fs/download?${q}`).then((r) => {
		if (!r.ok) throw new Error(`Core API ${r.status}: ${r.statusText}`)
		return r.blob()
	})
}

export function deleteFileOrFolder(
	base: string,
	id: string,
	path: string,
	recursive = false,
): Promise<{ ok: boolean }> {
	return apiFetch(`${base}/instances/${id}/fs`, {
		method: 'DELETE',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ path, recursive }),
	})
}

export function uploadFile(base: string, id: string, targetDir: string, file: File): UploadHandle {
	const url = `${base}/instances/${id}/fs/upload?path=${encodeURIComponent(targetDir)}`
	return xhrUpload(url, file)
}

// ── Backups ───────────────────────────────────────────────────────────────────

export function listBackups(base: string, id: string): Promise<CoreBackupsResponse> {
	return apiFetch(`${base}/instances/${id}/backups`)
}

export function createBackup(base: string, id: string, name?: string): Promise<CoreBackup> {
	return apiFetch(`${base}/instances/${id}/backups`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ name: name ?? null }),
	})
}

export function deleteBackup(base: string, id: string, backupId: string): Promise<{ ok: boolean }> {
	return apiFetch(`${base}/instances/${id}/backups/${backupId}`, { method: 'DELETE' })
}

export function deleteManyBackups(
	base: string,
	id: string,
	backupIds: string[],
): Promise<{ deleted: number }> {
	return apiFetch(`${base}/instances/${id}/backups/delete-many`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ ids: backupIds }),
	})
}

export function renameBackup(
	base: string,
	id: string,
	backupId: string,
	name: string,
): Promise<{ ok: boolean }> {
	return apiFetch(`${base}/instances/${id}/backups/${backupId}`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ name }),
	})
}

export function lockBackup(
	base: string,
	id: string,
	backupId: string,
	locked: boolean,
): Promise<{ ok: boolean }> {
	return apiFetch(`${base}/instances/${id}/backups/${backupId}/lock`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ locked }),
	})
}

export function restoreBackup(
	base: string,
	id: string,
	backupId: string,
): Promise<{ ok: boolean }> {
	return apiFetch(`${base}/instances/${id}/backups/${backupId}/restore`, { method: 'POST' })
}

export function getBackupSchedule(base: string, id: string): Promise<CoreBackupSchedule> {
	return apiFetch(`${base}/instances/${id}/backups/schedule`)
}

export function setBackupSchedule(
	base: string,
	id: string,
	schedule: CoreBackupScheduleBody,
): Promise<{ ok: boolean }> {
	return apiFetch(`${base}/instances/${id}/backups/schedule`, {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(schedule),
	})
}

// ── Internal: XHR upload ──────────────────────────────────────────────────────

function xhrUpload(url: string, file: File): UploadHandle {
	const progressCallbacks: Array<(percent: number) => void> = []
	let resolve!: () => void
	let reject!: (e: Error) => void
	const done = new Promise<void>((res, rej) => {
		resolve = res
		reject = rej
	})

	const xhr = new XMLHttpRequest()
	xhr.open('POST', url)
	xhr.upload.onprogress = (e) => {
		if (e.lengthComputable) {
			const pct = Math.round((e.loaded / e.total) * 100)
			progressCallbacks.forEach((cb) => cb(pct))
		}
	}
	xhr.onload = () => {
		if (xhr.status >= 200 && xhr.status < 300) resolve()
		else reject(new Error(`Core API ${xhr.status}: upload failed`))
	}
	xhr.onerror = () => reject(new Error('Core API: network error during upload'))

	const form = new FormData()
	form.append('file', file, file.name)
	xhr.send(form)

	return {
		onProgress: (cb) => progressCallbacks.push(cb),
		done,
		abort: () => xhr.abort(),
	}
}
