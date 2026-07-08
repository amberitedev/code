import type { CoreCallContext } from './context'
import type {
	CoreInstance,
	CoreInstanceSummary,
	CoreCreateInstanceBody,
	CorePatchInstanceBody,
	CoreChangeVersionBody,
	CoreStartupSettings,
	CoreStats,
	CorePlayers,
	CoreRconEnableResult,
	CoreServerQuery,
	CoreScheduledTask,
	CoreCreateTaskBody,
	CoreUpdateTaskBody,
	CoreMod,
	CoreFsListing,
	CoreBackup,
	CoreBackupsResponse,
	CoreBackupSchedule,
	CoreBackupScheduleBody,
	CoreNetworkStatus,
	CoreUploadSession,
	CoreModpackManifest,
	UploadHandle,
	CoreFsEntry,
	FsDownloadUrlResponse,
	UnzipOption,
	FsZipRequest,
	FsCopyRequest,
	CoreSetupStatus,
	CoreSetupRequest,
	CoreSetupResponse,
	CoreSetupDevResetResponse,
	CoreConnectionHandshakeRequest,
	CoreConnectionHandshakeResponse,
	CoreMetadata,
	CoreProjectionSyncResult,
	CoreMember,
	CoreAccessResponse,
	CoreAccessUpsertBody,
	CoreAccessPatchBody,
	CoreRole,
	CoreRoleConfiguration,
	SaveCoreRoleBody,
	CoreInvitation,
	CreateCoreInvitationBody,
	UpdateCoreInvitationBody,
	CoreActivityLogQuery,
	CoreActivityLogResponse,
	CoreSyncProfile,
	CoreCreateSyncProfileFromMrpackMetadata,
	CoreSyncSnapshot,
	CoreSyncEvent,
	CoreSyncSnapshotPublishResult,
	CoreSyncVersionStatus,
	CorePublishSyncSnapshotMetadata,
} from './types'
import { NetworkError, CoreApiError } from './errors'

const DEFAULT_TIMEOUT_MS = 15_000

function authHeaders(ctx: CoreCallContext): Record<string, string> {
	const h: Record<string, string> = {}
	if (ctx.token) h['Authorization'] = `Bearer ${ctx.token}`
	return h
}

async function request(ctx: CoreCallContext, url: string, init?: RequestInit): Promise<Response> {
	let res: Response
	const controller = new AbortController()
	const timeout = setTimeout(() => controller.abort(), ctx.timeoutMs ?? DEFAULT_TIMEOUT_MS)
	const abortFromCaller = () => controller.abort()
	ctx.signal?.addEventListener('abort', abortFromCaller, { once: true })
	try {
		res = await ctx.fetchFn(url, {
			...init,
			headers: { ...authHeaders(ctx), ...(init?.headers || {}) },
			signal: controller.signal,
		})
	} catch (e) {
		const reason = e instanceof Error ? e.message : String(e)
		throw new NetworkError(reason)
	} finally {
		clearTimeout(timeout)
		ctx.signal?.removeEventListener('abort', abortFromCaller)
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
	return res
}

async function apiFetch<T>(ctx: CoreCallContext, url: string, init?: RequestInit): Promise<T> {
	const res = await request(ctx, url, init)
	try {
		return await res.json()
	} catch (e) {
		const reason = e instanceof Error ? e.message : String(e)
		throw new NetworkError(`Invalid JSON response from ${url} — ${reason}`)
	}
}

async function rawFetch(ctx: CoreCallContext, url: string, init?: RequestInit): Promise<Response> {
	return request(ctx, url, init)
}

function instanceBaseUrl(ctx: CoreCallContext, path: string): string {
	return `${ctx.baseUrl}/instances/${encodeURIComponent(path)}`
}

export function getSetupStatus(ctx: CoreCallContext): Promise<CoreSetupStatus> {
	return apiFetch(ctx, `${ctx.baseUrl}/setup/status`)
}

export function completeSetup(
	ctx: CoreCallContext,
	body: CoreSetupRequest,
): Promise<CoreSetupResponse> {
	return apiFetch(ctx, `${ctx.baseUrl}/setup`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function devResetSetup(ctx: CoreCallContext): Promise<CoreSetupDevResetResponse> {
	return apiFetch(ctx, `${ctx.baseUrl}/setup/dev-reset`, { method: 'POST' })
}

export function connectionHandshake(
	ctx: CoreCallContext,
	body: CoreConnectionHandshakeRequest,
): Promise<CoreConnectionHandshakeResponse> {
	return apiFetch(ctx, `${ctx.baseUrl}/connection/handshake`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function getNetworkStatus(ctx: CoreCallContext): Promise<CoreNetworkStatus> {
	return apiFetch(ctx, `${ctx.baseUrl}/network/status`)
}

// ── Instances ─────────────────────────────────────────────────────────────────

export function listInstances(ctx: CoreCallContext): Promise<{ instances: CoreInstanceSummary[] }> {
	return apiFetch(ctx, `${ctx.baseUrl}/instances`)
}

export function getInstance(ctx: CoreCallContext, id: string): Promise<CoreInstance> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}`)
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
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}`, { method: 'DELETE' })
}

export function patchInstance(
	ctx: CoreCallContext,
	id: string,
	body: CorePatchInstanceBody,
): Promise<CoreInstance> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function getStartup(ctx: CoreCallContext, id: string): Promise<CoreStartupSettings> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/startup`)
}

// ── Instance lifecycle ────────────────────────────────────────────────────────

export function startInstance(ctx: CoreCallContext, id: string): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/start`, { method: 'POST' })
}

export function stopInstance(ctx: CoreCallContext, id: string): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/stop`, { method: 'POST' })
}

export function killInstance(ctx: CoreCallContext, id: string): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/kill`, { method: 'POST' })
}

export function restartInstance(ctx: CoreCallContext, id: string): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/restart`, { method: 'POST' })
}

export function repairInstance(ctx: CoreCallContext, id: string): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/repair`, { method: 'POST' })
}

export function changeInstanceVersion(
	ctx: CoreCallContext,
	id: string,
	body: CoreChangeVersionBody,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/change-version`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function sendCommand(
	ctx: CoreCallContext,
	id: string,
	command: string,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/command`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ command }),
	})
}

// ── WebSocket ticket ──────────────────────────────────────────────────────────

export function issueWsTicket(ctx: CoreCallContext): Promise<{ ticket: string }> {
	return apiFetch(ctx, `${ctx.baseUrl}/ws-token`, { method: 'POST' })
}

export function openEventStream(ctx: CoreCallContext): Promise<Response> {
	return rawFetch(ctx, `${ctx.baseUrl}/events`)
}

// ── Stats ─────────────────────────────────────────────────────────────────────

export function getStats(ctx: CoreCallContext, id: string): Promise<CoreStats> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/stats`)
}

// ── Mods ──────────────────────────────────────────────────────────────────────

export function listMods(ctx: CoreCallContext, id: string): Promise<{ mods: CoreMod[] }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/mods`)
}

export function addMod(ctx: CoreCallContext, id: string, versionId: string): Promise<CoreMod> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/mods`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ version_id: versionId }),
	})
}

export function addModProject(
	ctx: CoreCallContext,
	id: string,
	projectId: string,
): Promise<CoreMod> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/mods`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ project_id: projectId }),
	})
}

export function uploadModFile(ctx: CoreCallContext, id: string, file: File): UploadHandle {
	return xhrUpload(ctx, `${instanceBaseUrl(ctx, id)}/mods/upload`, file)
}

export function installModpackVersion(
	ctx: CoreCallContext,
	id: string,
	projectId: string,
	versionId: string,
): Promise<CoreModpackManifest> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/modpack/modrinth`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ project_id: projectId, version_id: versionId }),
	})
}

export function deleteMod(
	ctx: CoreCallContext,
	id: string,
	filename: string,
): Promise<{ ok: boolean; restart_required: boolean }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/mods/${encodeURIComponent(filename)}`, {
		method: 'DELETE',
	})
}

export function toggleMod(
	ctx: CoreCallContext,
	id: string,
	filename: string,
	enabled: boolean,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/mods/${encodeURIComponent(filename)}`, {
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
		`${instanceBaseUrl(ctx, id)}/mods/${encodeURIComponent(filename)}/update`,
		{
			method: 'PUT',
		},
	)
}

export function updateAllMods(
	ctx: CoreCallContext,
	id: string,
): Promise<{ updated: string[]; already_latest: string[]; failed: unknown[] }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/mods/update-all`, { method: 'POST' })
}

// ── Logs ──────────────────────────────────────────────────────────────────────

export function listLogs(ctx: CoreCallContext, id: string): Promise<{ logs: string[] }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/logs`)
}

export function readLog(ctx: CoreCallContext, id: string, filename: string): Promise<string> {
	return rawFetch(ctx, `${instanceBaseUrl(ctx, id)}/logs/${encodeURIComponent(filename)}`).then(
		(r) => r.text(),
	)
}

export function listCrashReports(
	ctx: CoreCallContext,
	id: string,
): Promise<{ crash_reports: string[] }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/crash-reports`)
}

export function readCrashReport(
	ctx: CoreCallContext,
	id: string,
	filename: string,
): Promise<string> {
	return rawFetch(
		ctx,
		`${instanceBaseUrl(ctx, id)}/crash-reports/${encodeURIComponent(filename)}`,
	).then((r) => r.text())
}

// ── Server properties ─────────────────────────────────────────────────────────

export function getProperties(
	ctx: CoreCallContext,
	id: string,
): Promise<{ properties: Record<string, string> }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/properties`)
}

export function patchProperties(
	ctx: CoreCallContext,
	id: string,
	updates: Record<string, string>,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/properties`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(updates),
	})
}

// ── RCON ──────────────────────────────────────────────────────────────────────

export function executeRcon(
	ctx: CoreCallContext,
	id: string,
	command: string,
): Promise<{ response: string }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/rcon`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ command }),
	})
}

export function enableRcon(ctx: CoreCallContext, id: string): Promise<CoreRconEnableResult> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/rcon/enable`, {
		method: 'POST',
	})
}

// ── Server Query ──────────────────────────────────────────────────────────────

export function queryInstance(ctx: CoreCallContext, id: string): Promise<CoreServerQuery> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/query`)
}

// ── Scheduled Tasks ───────────────────────────────────────────────────────────

export function listTasks(
	ctx: CoreCallContext,
	id: string,
): Promise<{ tasks: CoreScheduledTask[] }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/tasks`)
}

export function createTask(
	ctx: CoreCallContext,
	id: string,
	body: CoreCreateTaskBody,
): Promise<CoreScheduledTask> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/tasks`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function getTask(
	ctx: CoreCallContext,
	id: string,
	taskId: string,
): Promise<CoreScheduledTask> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/tasks/${encodeURIComponent(taskId)}`)
}

export function updateTask(
	ctx: CoreCallContext,
	id: string,
	taskId: string,
	body: CoreUpdateTaskBody,
): Promise<CoreScheduledTask> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/tasks/${encodeURIComponent(taskId)}`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function deleteTask(
	ctx: CoreCallContext,
	id: string,
	taskId: string,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/tasks/${encodeURIComponent(taskId)}`, {
		method: 'DELETE',
	})
}

// ── Players ───────────────────────────────────────────────────────────────────

export function listPlayers(ctx: CoreCallContext, id: string): Promise<CorePlayers> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/players`)
}

function playerAction(
	ctx: CoreCallContext,
	id: string,
	action: string,
	body: Record<string, unknown>,
): Promise<{ response: string }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/players/${action}`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function kickPlayer(
	ctx: CoreCallContext,
	id: string,
	name: string,
	reason?: string,
): Promise<{ response: string }> {
	return playerAction(ctx, id, 'kick', { name, reason })
}

export function opPlayer(
	ctx: CoreCallContext,
	id: string,
	name: string,
): Promise<{ response: string }> {
	return playerAction(ctx, id, 'op', { name })
}

export function deopPlayer(
	ctx: CoreCallContext,
	id: string,
	name: string,
): Promise<{ response: string }> {
	return playerAction(ctx, id, 'deop', { name })
}

export function banPlayer(
	ctx: CoreCallContext,
	id: string,
	name: string,
	reason?: string,
): Promise<{ response: string }> {
	return playerAction(ctx, id, 'ban', { name, reason })
}

export function pardonPlayer(
	ctx: CoreCallContext,
	id: string,
	name: string,
): Promise<{ response: string }> {
	return playerAction(ctx, id, 'pardon', { name })
}

export function banIp(
	ctx: CoreCallContext,
	id: string,
	ip: string,
	reason?: string,
): Promise<{ response: string }> {
	return playerAction(ctx, id, 'ban-ip', { ip, reason })
}

export function pardonIp(
	ctx: CoreCallContext,
	id: string,
	ip: string,
): Promise<{ response: string }> {
	return playerAction(ctx, id, 'pardon-ip', { ip })
}

export function addToWhitelist(
	ctx: CoreCallContext,
	id: string,
	name: string,
): Promise<{ response: string }> {
	return playerAction(ctx, id, 'whitelist', { name })
}

export function setWhitelistEnabled(
	ctx: CoreCallContext,
	id: string,
	enabled: boolean,
): Promise<{ response: string }> {
	return playerAction(ctx, id, 'whitelist/toggle', { enabled })
}

export function removeFromWhitelist(
	ctx: CoreCallContext,
	id: string,
	name: string,
): Promise<{ response: string }> {
	return apiFetch(
		ctx,
		`${instanceBaseUrl(ctx, id)}/players/whitelist/${encodeURIComponent(name)}`,
		{ method: 'DELETE' },
	)
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
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs?${q}`)
}

export function downloadFile(ctx: CoreCallContext, id: string, path: string): Promise<Blob> {
	const q = new URLSearchParams({ path })
	return rawFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs/download?${q}`).then((r) => r.blob())
}

export function deleteFileOrFolder(
	ctx: CoreCallContext,
	id: string,
	path: string,
	recursive = false,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs`, {
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
	const url = `${instanceBaseUrl(ctx, id)}/fs/upload?path=${encodeURIComponent(targetDir)}`
	return xhrUpload(ctx, url, file)
}

export function readFile(ctx: CoreCallContext, id: string, path: string): Promise<ArrayBuffer> {
	const q = new URLSearchParams({ path })
	return rawFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs/read?${q}`).then((r) => r.arrayBuffer())
}

export function writeFile(
	ctx: CoreCallContext,
	id: string,
	path: string,
	content: string | ArrayBuffer,
): Promise<{ ok: boolean }> {
	const q = new URLSearchParams({ path })
	const body = typeof content === 'string' ? new TextEncoder().encode(content) : content
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs/write?${q}`, {
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
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs/create?${q}`, { method: 'POST' })
}

export function createDir(
	ctx: CoreCallContext,
	id: string,
	path: string,
): Promise<{ ok: boolean }> {
	const q = new URLSearchParams({ path })
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs/mkdir?${q}`, { method: 'POST' })
}

export function moveEntry(
	ctx: CoreCallContext,
	id: string,
	from: string,
	to: string,
): Promise<{ ok: boolean }> {
	const q = new URLSearchParams({ from, to })
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs/move?${q}`, { method: 'PUT' })
}

export function unzipFile(
	ctx: CoreCallContext,
	id: string,
	path: string,
	option: UnzipOption,
): Promise<{ ok: boolean }> {
	const q = new URLSearchParams({ path })
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs/unzip?${q}`, {
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
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs/zip`, {
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
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs/copy`, {
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
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs/url?${q}`)
}

export function searchFiles(
	ctx: CoreCallContext,
	id: string,
	path: string,
	query: string,
	recursive: boolean,
): Promise<CoreFsEntry[]> {
	const q = new URLSearchParams({ path, query, recursive: String(recursive) })
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs/search?${q}`)
}

export async function createResumableUpload(
	ctx: CoreCallContext,
	id: string,
	path: string,
	length: number,
	sha256Hex?: string,
): Promise<CoreUploadSession> {
	const q = new URLSearchParams({ path })
	const headers: Record<string, string> = { 'Upload-Length': String(length) }
	if (sha256Hex) headers['Upload-Metadata'] = `sha256 ${base64FromText(sha256Hex)}`
	const response = await rawFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs/uploads?${q}`, {
		method: 'POST',
		headers,
	})
	return uploadSessionFromHeaders(response)
}

export async function getResumableUploadStatus(
	ctx: CoreCallContext,
	id: string,
	uploadId: string,
): Promise<CoreUploadSession> {
	const response = await rawFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs/uploads/${uploadId}`, {
		method: 'HEAD',
	})
	return uploadSessionFromHeaders(response, uploadId)
}

export async function appendResumableUpload(
	ctx: CoreCallContext,
	id: string,
	uploadId: string,
	offset: number,
	chunk: BodyInit,
	sha256DigestBase64?: string,
): Promise<CoreUploadSession> {
	const headers: Record<string, string> = { 'Upload-Offset': String(offset) }
	if (sha256DigestBase64) headers['Upload-Checksum'] = `sha256 ${sha256DigestBase64}`
	const response = await rawFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs/uploads/${uploadId}`, {
		method: 'PATCH',
		headers,
		body: chunk,
	})
	return uploadSessionFromHeaders(response, uploadId)
}

export function cancelResumableUpload(
	ctx: CoreCallContext,
	id: string,
	uploadId: string,
): Promise<Response> {
	return rawFetch(ctx, `${instanceBaseUrl(ctx, id)}/fs/uploads/${uploadId}`, {
		method: 'DELETE',
	})
}

// ── Backups ───────────────────────────────────────────────────────────────────

export function listBackups(ctx: CoreCallContext, id: string): Promise<CoreBackupsResponse> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/backups`)
}

export function createBackup(ctx: CoreCallContext, id: string, name?: string): Promise<CoreBackup> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/backups`, {
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
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/backups/${backupId}`, { method: 'DELETE' })
}

export function deleteManyBackups(
	ctx: CoreCallContext,
	id: string,
	backupIds: string[],
): Promise<{ deleted: number }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/backups/delete-many`, {
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
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/backups/${backupId}`, {
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
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/backups/${backupId}/lock`, {
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
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/backups/${backupId}/restore`, {
		method: 'POST',
	})
}

export function getBackupSchedule(ctx: CoreCallContext, id: string): Promise<CoreBackupSchedule> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/backups/schedule`)
}

export function setBackupSchedule(
	ctx: CoreCallContext,
	id: string,
	schedule: CoreBackupScheduleBody,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/backups/schedule`, {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(schedule),
	})
}

// ── Social / Core Identity ────────────────────────────────────────────────────

export function getCoreMetadata(ctx: CoreCallContext): Promise<CoreMetadata> {
	return apiFetch(ctx, `${ctx.baseUrl}/core`)
}

export function updateCoreMetadata(
	ctx: CoreCallContext,
	body: Partial<CoreMetadata>,
): Promise<CoreMetadata> {
	return apiFetch(ctx, `${ctx.baseUrl}/core`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function listCoreMembers(ctx: CoreCallContext): Promise<{ members: CoreMember[] }> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/members`)
}

export function upsertCoreMember(
	ctx: CoreCallContext,
	body: Partial<CoreMember> & { user_id: string },
): Promise<CoreMember> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/members`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function removeCoreMember(ctx: CoreCallContext, userId: string): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/members/${encodeURIComponent(userId)}`, {
		method: 'DELETE',
	})
}

export function resyncCoreProjection(ctx: CoreCallContext): Promise<CoreProjectionSyncResult> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/projection/resync`, { method: 'POST' })
}

export function listCoreAccess(ctx: CoreCallContext): Promise<CoreAccessResponse> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/access`)
}

export function grantCoreAccess(
	ctx: CoreCallContext,
	body: CoreAccessUpsertBody,
): Promise<CoreMember> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/access`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function updateCoreAccess(
	ctx: CoreCallContext,
	userId: string,
	body: CoreAccessPatchBody,
): Promise<CoreMember> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/access/${encodeURIComponent(userId)}`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function removeCoreAccess(ctx: CoreCallContext, userId: string): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/access/${encodeURIComponent(userId)}`, {
		method: 'DELETE',
	})
}

export function getCoreRoles(ctx: CoreCallContext): Promise<CoreRoleConfiguration> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/roles`)
}

export function saveCoreRole(ctx: CoreCallContext, body: SaveCoreRoleBody): Promise<CoreRole> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/roles`, {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function retireCoreRole(ctx: CoreCallContext, id: string): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/roles/${encodeURIComponent(id)}`, { method: 'DELETE' })
}

export function createCoreInvitation(
	ctx: CoreCallContext,
	body: CreateCoreInvitationBody,
): Promise<CoreInvitation> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/invitations`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function updateCoreInvitation(
	ctx: CoreCallContext,
	id: string,
	body: UpdateCoreInvitationBody,
): Promise<CoreInvitation> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/invitations/${encodeURIComponent(id)}`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function listCoreInvitations(
	ctx: CoreCallContext,
): Promise<{ invitations: CoreInvitation[] }> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/invitations`)
}
export function listMyCoreInvitations(
	ctx: CoreCallContext,
): Promise<{ invitations: CoreInvitation[] }> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/invitations/mine`)
}
export function reviewCoreInvitation(
	ctx: CoreCallContext,
	id: string,
	accept: boolean,
): Promise<CoreInvitation> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/invitations/${encodeURIComponent(id)}/review`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ accept }),
	})
}
export function revokeCoreInvitation(ctx: CoreCallContext, id: string): Promise<CoreInvitation> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/invitations/${encodeURIComponent(id)}`, {
		method: 'DELETE',
	})
}
export function respondToCoreInvitation(
	ctx: CoreCallContext,
	id: string,
	accept: boolean,
): Promise<CoreInvitation> {
	return apiFetch(ctx, `${ctx.baseUrl}/core/invitations/${encodeURIComponent(id)}/respond`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ accept }),
	})
}

export function listInstanceAccess(ctx: CoreCallContext, id: string): Promise<CoreAccessResponse> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/access`)
}

export function grantInstanceAccess(
	ctx: CoreCallContext,
	id: string,
	body: CoreAccessUpsertBody,
): Promise<unknown> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/access`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function updateInstanceAccess(
	ctx: CoreCallContext,
	id: string,
	userId: string,
	body: CoreAccessPatchBody,
): Promise<unknown> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/access/${encodeURIComponent(userId)}`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function removeInstanceAccess(
	ctx: CoreCallContext,
	id: string,
	userId: string,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/access/${encodeURIComponent(userId)}`, {
		method: 'DELETE',
	})
}

export function listActivity(
	ctx: CoreCallContext,
	query: CoreActivityLogQuery = {},
): Promise<CoreActivityLogResponse> {
	const q = new URLSearchParams()
	for (const [key, value] of Object.entries(query)) {
		if (value !== undefined && value !== null) q.set(key, String(value))
	}
	return apiFetch(ctx, `${ctx.baseUrl}/activity${q.size ? `?${q}` : ''}`)
}

export function listInstanceActivity(
	ctx: CoreCallContext,
	id: string,
	query: CoreActivityLogQuery = {},
): Promise<CoreActivityLogResponse> {
	const q = new URLSearchParams()
	for (const [key, value] of Object.entries(query)) {
		if (value !== undefined && value !== null && key !== 'instance_id') q.set(key, String(value))
	}
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/activity${q.size ? `?${q}` : ''}`)
}

// ── Sync Profiles ─────────────────────────────────────────────────────────────

export function listSyncProfiles(ctx: CoreCallContext): Promise<{ profiles: CoreSyncProfile[] }> {
	return apiFetch(ctx, `${ctx.baseUrl}/sync/profiles`)
}

export function registerSyncProfile(
	ctx: CoreCallContext,
	body: Partial<CoreSyncProfile> & { name: string },
): Promise<CoreSyncProfile> {
	return apiFetch(ctx, `${ctx.baseUrl}/sync/profiles`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	})
}

export function removeSyncProfile(
	ctx: CoreCallContext,
	profileId: string,
): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${ctx.baseUrl}/sync/profiles/${encodeURIComponent(profileId)}`, {
		method: 'DELETE',
	})
}

export function createSyncProfileFromMrpack(
	ctx: CoreCallContext,
	file: File,
	metadata?: CoreCreateSyncProfileFromMrpackMetadata,
): Promise<CoreSyncSnapshotPublishResult> {
	const form = new FormData()
	form.append('mrpack', file, file.name)
	if (metadata) form.append('metadata', JSON.stringify(metadata))
	return apiFetch(ctx, `${ctx.baseUrl}/sync/profiles/from-mrpack`, {
		method: 'POST',
		body: form,
	})
}

export function publishSyncSnapshot(
	ctx: CoreCallContext,
	profileId: string,
	file: File,
	metadata?: string | CorePublishSyncSnapshotMetadata,
): Promise<CoreSyncSnapshotPublishResult> {
	const form = new FormData()
	form.append('mrpack', file, file.name)
	const notes = typeof metadata === 'string' ? metadata : metadata?.notes
	if (notes) form.append('notes', notes)
	if (metadata && typeof metadata !== 'string') form.append('metadata', JSON.stringify(metadata))
	return apiFetch(ctx, `${ctx.baseUrl}/sync/profiles/${encodeURIComponent(profileId)}/snapshots`, {
		method: 'POST',
		body: form,
	})
}

export function listSyncSnapshots(
	ctx: CoreCallContext,
	profileId: string,
): Promise<{ snapshots: CoreSyncSnapshot[] }> {
	return apiFetch(ctx, `${ctx.baseUrl}/sync/profiles/${encodeURIComponent(profileId)}/snapshots`)
}

export function listSyncEvents(
	ctx: CoreCallContext,
	profileId: string,
): Promise<{ events: CoreSyncEvent[] }> {
	return apiFetch(ctx, `${ctx.baseUrl}/sync/profiles/${encodeURIComponent(profileId)}/events`)
}

export function checkSyncVersion(
	ctx: CoreCallContext,
	profileId: string,
): Promise<CoreSyncVersionStatus> {
	return apiFetch(
		ctx,
		`${ctx.baseUrl}/sync/profiles/${encodeURIComponent(profileId)}/check-version`,
	)
}

export function downloadSyncSnapshot(
	ctx: CoreCallContext,
	profileId: string,
	snapshotId: string,
): Promise<Blob> {
	return rawFetch(
		ctx,
		`${ctx.baseUrl}/sync/profiles/${encodeURIComponent(profileId)}/snapshots/${encodeURIComponent(snapshotId)}/download`,
	).then((r) => r.blob())
}

// ── Modpack (additional) ──────────────────────────────────────────────────────

export async function getModpack(
	ctx: CoreCallContext,
	id: string,
): Promise<CoreModpackManifest | null> {
	try {
		return await apiFetch<CoreModpackManifest>(ctx, `${instanceBaseUrl(ctx, id)}/modpack`)
	} catch (e) {
		if (e instanceof CoreApiError && e.status === 404) return null
		throw e
	}
}

export function removeModpack(ctx: CoreCallContext, id: string): Promise<{ ok: boolean }> {
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/modpack`, { method: 'DELETE' })
}

export function exportModpack(ctx: CoreCallContext, id: string): Promise<Blob> {
	return rawFetch(ctx, `${instanceBaseUrl(ctx, id)}/modpack/export`).then((r) => r.blob())
}

export function installModpackFile(
	ctx: CoreCallContext,
	id: string,
	file: File,
): Promise<CoreModpackManifest> {
	const form = new FormData()
	form.append('file', file, file.name)
	return apiFetch(ctx, `${instanceBaseUrl(ctx, id)}/modpack/upload`, {
		method: 'POST',
		body: form,
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
	xhr.onabort = () => reject(new NetworkError('Upload aborted'))
	xhr.timeout = ctx.timeoutMs ?? DEFAULT_TIMEOUT_MS
	xhr.ontimeout = () => reject(new NetworkError('Upload timed out'))

	const form = new FormData()
	form.append('file', file, file.name)
	xhr.send(form)

	return {
		onProgress: (cb) => progressCallbacks.push(cb),
		done,
		abort: () => xhr.abort(),
	}
}

function uploadSessionFromHeaders(response: Response, uploadId?: string): CoreUploadSession {
	const location = response.headers.get('Location') ?? ''
	const id = uploadId ?? location.split('/').pop() ?? ''
	return {
		id,
		location,
		offset: Number(response.headers.get('Upload-Offset') ?? 0),
		length: Number(response.headers.get('Upload-Length') ?? 0),
	}
}

function base64FromText(value: string): string {
	const bytes = new TextEncoder().encode(value)
	let binary = ''
	for (const byte of bytes) binary += String.fromCharCode(byte)
	return btoa(binary)
}
