/**
 * CoreApiClient — primary entry point for all Copal API calls.
 *
 * Accepts a PlatformAdapter and talks directly to Core. Relay behavior is not
 * implicit; asynchronous delivery uses explicitly declared message transports.
 *
 * Key methods: getInstance, listInstances, createInstance, deleteInstance, renameInstance,
 * updateJavaVersion, start/stop/kill/restart, issueWsTicket, openConsole, getStats,
 * listMods/addMod/addModProject/uploadModFile/deleteMod/toggleMod/updateMod/updateAllMods,
 * listLogs/readLog, listCrashReports/readCrashReport,
 * getProperties/patchProperties, listDirectory/downloadFile/deleteFileOrFolder/uploadFile,
 * listBackups/createBackup/renameBackup/deleteBackup/deleteManyBackups/lockBackup/restoreBackup,
 * getBackupSchedule/setBackupSchedule,
 * getCoreMetadata/updateCoreMetadata/listCoreMembers/upsertCoreMember/removeCoreMember,
 * listSyncProfiles/registerSyncProfile/removeSyncProfile,
 * getModpack/removeModpack/exportModpack/installModpackFile.
 */

import type { PlatformAdapter } from './adapter'
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
	CoreBackupsResponse,
	CoreBackup,
	CoreBackupSchedule,
	CoreBackupScheduleBody,
	UploadHandle,
	FsDownloadUrlResponse,
	UnzipOption,
	FsZipRequest,
	FsCopyRequest,
	CoreSetupStatus,
	CoreSetupRequest,
	CoreSetupResponse,
	CoreInstanceEvent,
	CoreModpackManifest,
	CoreMetadata,
	CoreMember,
	CoreAccessResponse,
	CoreAccessUpsertBody,
	CoreAccessPatchBody,
	CoreActivityLogQuery,
	CoreActivityLogResponse,
	CoreSyncProfile,
	CoreCreateSyncProfileFromMrpackMetadata,
	CoreSyncSnapshot,
	CoreSyncEvent,
	CoreSyncSnapshotPublishResult,
	CoreSyncVersionStatus,
} from './types'
import * as api from './api'
import { CoreWsConnection } from './ws'
import { CoreOfflineError } from './errors'
import { CoreConnectionMonitor, type ConnectionStatus } from './monitor'
import { CommunicationPipeline, type CommunicationPipelineOptions } from './pipeline'
import { resolveCoreEndpointKey } from './endpoint-policies'
import type { CommunicationPolicyOverride } from './pipeline-types'

export class CoreApiClient {
	public monitor: CoreConnectionMonitor | null = null
	public readonly pipeline: CommunicationPipeline
	private coreUrlPromise: Promise<string | null> | null = null

	constructor(
		public readonly adapter: PlatformAdapter,
		private readonly options: { timeoutMs?: number; pipeline?: CommunicationPipelineOptions } = {},
	) {
		this.pipeline = new CommunicationPipeline(adapter, options.pipeline)
		this.monitor = new CoreConnectionMonitor(adapter)
	}

	withPolicy(policy: CommunicationPolicyOverride): CoreApiClient {
		return new CoreApiClient(this.adapter, {
			...this.options,
			pipeline: {
				...this.options.pipeline,
				defaultPolicy: {
					...this.options.pipeline?.defaultPolicy,
					...policy,
				},
			},
		})
	}

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

	clearCoreUrlCache(): void {
		this.coreUrlPromise = null
	}

	private async direct<T>(
		keyOrFn: string | ((ctx: CoreCallContext) => Promise<T>),
		maybeFn?: (ctx: CoreCallContext) => Promise<T>,
		policy?: CommunicationPolicyOverride,
	): Promise<T> {
		const key = typeof keyOrFn === 'string' ? keyOrFn : 'core.default'
		const fn = typeof keyOrFn === 'string' ? maybeFn : keyOrFn
		if (!fn) throw new Error(`Missing Core API function for ${key}`)

		return this.pipeline.callValue({
			key,
			surface: 'core',
			policy,
			execute: async (signal, resolvedPolicy) => {
				const coreUrl = await this.getCoreUrlCached()
				if (!coreUrl) throw new CoreOfflineError()
				const token = await this.adapter.getCurrentJwt()
				const ctx: CoreCallContext = {
					baseUrl: coreUrl,
					token,
					fetchFn: this.adapter.fetchFn,
					timeoutMs: this.options.timeoutMs ?? resolvedPolicy.timeoutMs,
					signal,
				}
				return fn(ctx)
			},
		})
	}

	private async request<T>(
		directFn: (ctx: CoreCallContext) => Promise<T>,
		relayPayload: {
			method?: string
			path?: string
			policy?: CommunicationPolicyOverride
			[key: string]: unknown
		},
	): Promise<T> {
		const key =
			relayPayload.method && relayPayload.path
				? resolveCoreEndpointKey(relayPayload.method, relayPayload.path)
				: 'core.default'
		return await this.direct(key, directFn, relayPayload.policy)
	}

	getSetupStatus(): Promise<CoreSetupStatus> {
		return this.direct('core.setup.status', api.getSetupStatus)
	}

	completeSetup(body: CoreSetupRequest): Promise<CoreSetupResponse> {
		return this.direct('core.setup.complete', (ctx) => api.completeSetup(ctx, body))
	}

	completeSetupAt(coreUrl: string, body: CoreSetupRequest): Promise<CoreSetupResponse> {
		return this.pipeline.callValue({
			key: 'core.setup.complete',
			surface: 'core',
			execute: async (signal, resolvedPolicy) => {
				const token = await this.adapter.getCurrentJwt()
				return api.completeSetup(
					{
						baseUrl: coreUrl.replace(/\/$/, ''),
						token,
						fetchFn: this.adapter.fetchFn,
						timeoutMs: this.options.timeoutMs ?? resolvedPolicy.timeoutMs,
						signal,
					},
					body,
				)
			},
		})
	}

	connect(): Promise<ConnectionStatus> {
		return this.monitor?.checkNow() ?? new CoreConnectionMonitor(this.adapter).checkNow()
	}

	async completeLocalSetup(ownerUserId: string, authJwksUrl: string): Promise<CoreSetupResponse> {
		const localSetupSecret = await this.adapter.getLocalSetupSecret?.()
		if (!localSetupSecret) throw new Error('Local Core setup secret is not available')

		return this.completeSetup({
			local_setup_secret: localSetupSecret,
			convex_url: this.adapter.convexUrl,
			auth_jwks_url: authJwksUrl,
			owner_user_id: ownerUserId,
		})
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

	/** Set RAM bounds (MB). Applied on next start. */
	updateMemory(id: string, minMb: number, maxMb: number): Promise<CoreInstance> {
		const memory = { min_mb: minMb, max_mb: maxMb }
		return this.request((ctx) => api.patchInstance(ctx, id, { memory }), {
			method: 'PATCH',
			path: `/instances/${id}`,
			body: { memory },
		})
	}

	/**
	 * Update the startup overrides. Pass `null` for a field to clear that override
	 * and fall back to Core's defaults; omit a field to leave it unchanged.
	 */
	updateStartup(
		id: string,
		overrides: { jvm_args?: string | null; server_args?: string | null },
	): Promise<CoreInstance> {
		return this.request((ctx) => api.patchInstance(ctx, id, overrides), {
			method: 'PATCH',
			path: `/instances/${id}`,
			body: overrides,
		})
	}

	/** Current startup configuration plus rendered default/effective launch commands. */
	getStartup(id: string): Promise<CoreStartupSettings> {
		return this.request((ctx) => api.getStartup(ctx, id), {
			method: 'GET',
			path: `/instances/${id}/startup`,
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

	repair(id: string): Promise<void> {
		return this.request((ctx) => api.repairInstance(ctx, id).then(() => undefined), {
			method: 'POST',
			path: `/instances/${id}/repair`,
		})
	}

	changeVersion(id: string, body: CoreChangeVersionBody): Promise<void> {
		return this.request((ctx) => api.changeInstanceVersion(ctx, id, body).then(() => undefined), {
			method: 'POST',
			path: `/instances/${id}/change-version`,
			body,
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

	openEvents(): Promise<CoreEventStream> {
		return this.direct('core.events.open', async (ctx) => {
			const response = await api.openEventStream(ctx)
			return new CoreEventStream(response)
		})
	}

	async openConsole(instanceId: string, ticket: string): Promise<CoreWsConnection> {
		return this.pipeline.callValue({
			key: 'core.console.open',
			surface: 'core',
			execute: async () => {
				const coreUrl = await this.getCoreUrlCached()
				if (!coreUrl) throw new CoreOfflineError()
				const wsUrl =
					coreUrl.replace(/^http/, 'ws') +
					`/instances/${encodeURIComponent(instanceId)}/console?ticket=${encodeURIComponent(ticket)}`
				return new CoreWsConnection(wsUrl)
			},
		})
	}

	// ── Stats ───────────────────────────────────────────────────────────────

	getStats(id: string): Promise<CoreStats> {
		return this.request((ctx) => api.getStats(ctx, id), {
			method: 'GET',
			path: `/instances/${id}/stats`,
		})
	}

	// ── RCON ────────────────────────────────────────────────────────────────

	/** Execute a raw RCON command against a running instance. */
	executeRcon(id: string, command: string): Promise<{ response: string }> {
		return this.request((ctx) => api.executeRcon(ctx, id, command), {
			method: 'POST',
			path: `/instances/${id}/rcon`,
			body: { command },
		})
	}

	/** Enable RCON on an instance (patches server.properties, may need restart). */
	enableRcon(id: string): Promise<CoreRconEnableResult> {
		return this.request((ctx) => api.enableRcon(ctx, id), {
			method: 'POST',
			path: `/instances/${id}/rcon/enable`,
		})
	}

	/** Query a running instance via Server List Ping (MOTD, version, players, latency). */
	queryInstance(id: string): Promise<CoreServerQuery> {
		return this.request((ctx) => api.queryInstance(ctx, id), {
			method: 'GET',
			path: `/instances/${id}/query`,
		})
	}

	// ── Scheduled Tasks ─────────────────────────────────────────────────────

	listTasks(id: string): Promise<CoreScheduledTask[]> {
		return this.request((ctx) => api.listTasks(ctx, id).then((r) => r.tasks), {
			method: 'GET',
			path: `/instances/${id}/tasks`,
		})
	}

	createTask(id: string, body: CoreCreateTaskBody): Promise<CoreScheduledTask> {
		return this.request((ctx) => api.createTask(ctx, id, body), {
			method: 'POST',
			path: `/instances/${id}/tasks`,
			body,
		})
	}

	getTask(id: string, taskId: string): Promise<CoreScheduledTask> {
		return this.request((ctx) => api.getTask(ctx, id, taskId), {
			method: 'GET',
			path: `/instances/${id}/tasks/${encodeURIComponent(taskId)}`,
		})
	}

	updateTask(id: string, taskId: string, body: CoreUpdateTaskBody): Promise<CoreScheduledTask> {
		return this.request((ctx) => api.updateTask(ctx, id, taskId, body), {
			method: 'PATCH',
			path: `/instances/${id}/tasks/${encodeURIComponent(taskId)}`,
			body,
		})
	}

	deleteTask(id: string, taskId: string): Promise<void> {
		return this.request((ctx) => api.deleteTask(ctx, id, taskId).then(() => undefined), {
			method: 'DELETE',
			path: `/instances/${id}/tasks/${encodeURIComponent(taskId)}`,
		})
	}

	// ── Players ─────────────────────────────────────────────────────────────

	/** Full player-management snapshot (online list + whitelist/ops/bans). */
	listPlayers(id: string): Promise<CorePlayers> {
		return this.request((ctx) => api.listPlayers(ctx, id), {
			method: 'GET',
			path: `/instances/${id}/players`,
		})
	}

	kickPlayer(id: string, name: string, reason?: string): Promise<{ response: string }> {
		return this.request((ctx) => api.kickPlayer(ctx, id, name, reason), {
			method: 'POST',
			path: `/instances/${id}/players/kick`,
			body: { name, reason },
		})
	}

	opPlayer(id: string, name: string): Promise<{ response: string }> {
		return this.request((ctx) => api.opPlayer(ctx, id, name), {
			method: 'POST',
			path: `/instances/${id}/players/op`,
			body: { name },
		})
	}

	deopPlayer(id: string, name: string): Promise<{ response: string }> {
		return this.request((ctx) => api.deopPlayer(ctx, id, name), {
			method: 'POST',
			path: `/instances/${id}/players/deop`,
			body: { name },
		})
	}

	banPlayer(id: string, name: string, reason?: string): Promise<{ response: string }> {
		return this.request((ctx) => api.banPlayer(ctx, id, name, reason), {
			method: 'POST',
			path: `/instances/${id}/players/ban`,
			body: { name, reason },
		})
	}

	pardonPlayer(id: string, name: string): Promise<{ response: string }> {
		return this.request((ctx) => api.pardonPlayer(ctx, id, name), {
			method: 'POST',
			path: `/instances/${id}/players/pardon`,
			body: { name },
		})
	}

	banIp(id: string, ip: string, reason?: string): Promise<{ response: string }> {
		return this.request((ctx) => api.banIp(ctx, id, ip, reason), {
			method: 'POST',
			path: `/instances/${id}/players/ban-ip`,
			body: { ip, reason },
		})
	}

	pardonIp(id: string, ip: string): Promise<{ response: string }> {
		return this.request((ctx) => api.pardonIp(ctx, id, ip), {
			method: 'POST',
			path: `/instances/${id}/players/pardon-ip`,
			body: { ip },
		})
	}

	addToWhitelist(id: string, name: string): Promise<{ response: string }> {
		return this.request((ctx) => api.addToWhitelist(ctx, id, name), {
			method: 'POST',
			path: `/instances/${id}/players/whitelist`,
			body: { name },
		})
	}

	removeFromWhitelist(id: string, name: string): Promise<{ response: string }> {
		return this.request((ctx) => api.removeFromWhitelist(ctx, id, name), {
			method: 'DELETE',
			path: `/instances/${id}/players/whitelist/${encodeURIComponent(name)}`,
		})
	}

	setWhitelistEnabled(id: string, enabled: boolean): Promise<{ response: string }> {
		return this.request((ctx) => api.setWhitelistEnabled(ctx, id, enabled), {
			method: 'POST',
			path: `/instances/${id}/players/whitelist/toggle`,
			body: { enabled },
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

	addModProject(id: string, projectId: string): Promise<CoreMod> {
		return this.request((ctx) => api.addModProject(ctx, id, projectId), {
			method: 'POST',
			path: `/instances/${id}/mods`,
			body: { project_id: projectId },
		})
	}

	installModpackVersion(
		id: string,
		projectId: string,
		versionId: string,
	): Promise<CoreModpackManifest> {
		return this.request((ctx) => api.installModpackVersion(ctx, id, projectId, versionId), {
			method: 'POST',
			path: `/instances/${id}/modpack/modrinth`,
			body: { project_id: projectId, version_id: versionId },
		})
	}

	uploadModFile(id: string, file: File): UploadHandle {
		const policy = this.pipeline.resolvePolicy('core.mods.upload')
		const progressCallbacks: Array<(percent: number) => void> = []
		let innerAbort = () => {}
		let aborted = false
		const done = (async () => {
			const coreUrl = await this.getCoreUrlCached()
			const token = await this.adapter.getCurrentJwt()
			if (!coreUrl) throw new CoreOfflineError()
			if (aborted) return
			const ctx: CoreCallContext = {
				baseUrl: coreUrl,
				token,
				fetchFn: this.adapter.fetchFn,
				timeoutMs: this.options.timeoutMs ?? policy.timeoutMs,
			}
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
		const policy = this.pipeline.resolvePolicy('core.fs.upload')
		const progressCallbacks: Array<(percent: number) => void> = []
		let innerAbort = () => {}
		let aborted = false
		const done = (async () => {
			const coreUrl = await this.getCoreUrlCached()
			const token = await this.adapter.getCurrentJwt()
			if (!coreUrl) throw new CoreOfflineError()
			if (aborted) return
			const ctx: CoreCallContext = {
				baseUrl: coreUrl,
				token,
				fetchFn: this.adapter.fetchFn,
				timeoutMs: this.options.timeoutMs ?? policy.timeoutMs,
			}
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

	// ── Social / Core Identity ───────────────────────────────────────────────

	getCoreMetadata(): Promise<CoreMetadata> {
		return this.direct('core.metadata.get', api.getCoreMetadata)
	}

	updateCoreMetadata(body: Partial<CoreMetadata>): Promise<CoreMetadata> {
		return this.direct('core.metadata.update', (ctx) => api.updateCoreMetadata(ctx, body))
	}

	listCoreMembers(): Promise<CoreMember[]> {
		return this.direct('core.members.list', (ctx) =>
			api.listCoreMembers(ctx).then((r) => r.members),
		)
	}

	upsertCoreMember(body: Partial<CoreMember> & { user_id: string }): Promise<CoreMember> {
		return this.direct('core.members.upsert', (ctx) => api.upsertCoreMember(ctx, body))
	}

	removeCoreMember(userId: string): Promise<void> {
		return this.direct('core.members.remove', (ctx) =>
			api.removeCoreMember(ctx, userId).then(() => undefined),
		)
	}

	listCoreAccess(): Promise<CoreAccessResponse> {
		return this.direct('core.access.list', api.listCoreAccess)
	}

	grantCoreAccess(body: CoreAccessUpsertBody): Promise<CoreMember> {
		return this.direct('core.access.grant', (ctx) => api.grantCoreAccess(ctx, body))
	}

	updateCoreAccess(userId: string, body: CoreAccessPatchBody): Promise<CoreMember> {
		return this.direct('core.access.update', (ctx) => api.updateCoreAccess(ctx, userId, body))
	}

	removeCoreAccess(userId: string): Promise<void> {
		return this.direct('core.access.remove', (ctx) =>
			api.removeCoreAccess(ctx, userId).then(() => undefined),
		)
	}

	listInstanceAccess(id: string): Promise<CoreAccessResponse> {
		return this.request((ctx) => api.listInstanceAccess(ctx, id), {
			method: 'GET',
			path: `/instances/${id}/access`,
		})
	}

	grantInstanceAccess(id: string, body: CoreAccessUpsertBody): Promise<void> {
		return this.request((ctx) => api.grantInstanceAccess(ctx, id, body).then(() => undefined), {
			method: 'POST',
			path: `/instances/${id}/access`,
			body,
		})
	}

	updateInstanceAccess(id: string, userId: string, body: CoreAccessPatchBody): Promise<void> {
		return this.request(
			(ctx) => api.updateInstanceAccess(ctx, id, userId, body).then(() => undefined),
			{
				method: 'PATCH',
				path: `/instances/${id}/access/${encodeURIComponent(userId)}`,
				body,
			},
		)
	}

	removeInstanceAccess(id: string, userId: string): Promise<void> {
		return this.request(
			(ctx) => api.removeInstanceAccess(ctx, id, userId).then(() => undefined),
			{ method: 'DELETE', path: `/instances/${id}/access/${encodeURIComponent(userId)}` },
		)
	}

	listActivity(query?: CoreActivityLogQuery): Promise<CoreActivityLogResponse> {
		return this.direct('core.activity.list', (ctx) => api.listActivity(ctx, query))
	}

	listInstanceActivity(
		id: string,
		query?: CoreActivityLogQuery,
	): Promise<CoreActivityLogResponse> {
		return this.request((ctx) => api.listInstanceActivity(ctx, id, query), {
			method: 'GET',
			path: `/instances/${id}/activity`,
		})
	}

	// ── Sync Profiles ────────────────────────────────────────────────────────

	listSyncProfiles(): Promise<CoreSyncProfile[]> {
		return this.direct('core.sync_profiles.list', (ctx) =>
			api.listSyncProfiles(ctx).then((r) => r.profiles),
		)
	}

	registerSyncProfile(body: Partial<CoreSyncProfile> & { name: string }): Promise<CoreSyncProfile> {
		return this.direct('core.sync_profiles.register', (ctx) => api.registerSyncProfile(ctx, body))
	}

	removeSyncProfile(profileId: string): Promise<void> {
		return this.direct('core.sync_profiles.remove', (ctx) =>
			api.removeSyncProfile(ctx, profileId).then(() => undefined),
		)
	}

	createSyncProfileFromMrpack(
		file: File,
		metadata?: CoreCreateSyncProfileFromMrpackMetadata,
	): Promise<CoreSyncSnapshotPublishResult> {
		return this.direct('core.sync_profiles.create_from_mrpack', (ctx) =>
			api.createSyncProfileFromMrpack(ctx, file, metadata),
		)
	}

	publishSyncSnapshot(
		profileId: string,
		file: File,
		notes?: string,
	): Promise<CoreSyncSnapshotPublishResult> {
		return this.direct('core.sync_snapshots.publish', (ctx) =>
			api.publishSyncSnapshot(ctx, profileId, file, notes),
		)
	}

	listSyncSnapshots(profileId: string): Promise<CoreSyncSnapshot[]> {
		return this.direct('core.sync_snapshots.list', (ctx) =>
			api.listSyncSnapshots(ctx, profileId).then((r) => r.snapshots),
		)
	}

	listSyncEvents(profileId: string): Promise<CoreSyncEvent[]> {
		return this.direct('core.sync_events.list', (ctx) =>
			api.listSyncEvents(ctx, profileId).then((r) => r.events),
		)
	}

	checkSyncVersion(profileId: string): Promise<CoreSyncVersionStatus> {
		return this.direct('core.sync_version.check', (ctx) => api.checkSyncVersion(ctx, profileId))
	}

	downloadSyncSnapshot(profileId: string, snapshotId: string): Promise<Blob> {
		return this.direct('core.sync_snapshots.download', (ctx) =>
			api.downloadSyncSnapshot(ctx, profileId, snapshotId),
		)
	}

	// ── Modpack (additional) ─────────────────────────────────────────────────

	getModpack(id: string): Promise<CoreModpackManifest | null> {
		return this.direct('core.modpack.get', (ctx) => api.getModpack(ctx, id))
	}

	removeModpack(id: string): Promise<void> {
		return this.direct('core.modpack.remove', (ctx) =>
			api.removeModpack(ctx, id).then(() => undefined),
		)
	}

	exportModpack(id: string): Promise<Blob> {
		return this.direct('core.modpack.export', (ctx) => api.exportModpack(ctx, id))
	}

	installModpackFile(id: string, file: File): Promise<CoreModpackManifest> {
		return this.direct('core.modpack.install_file', (ctx) => api.installModpackFile(ctx, id, file))
	}
}

export class CoreEventStream {
	private abortController = new AbortController()
	private listeners: Array<(event: CoreInstanceEvent) => void> = []
	private errorListeners: Array<(error: unknown) => void> = []
	private closeListeners: Array<() => void> = []

	constructor(private readonly response: Response) {
		void this.readLoop()
	}

	onEvent(cb: (event: CoreInstanceEvent) => void): () => void {
		this.listeners.push(cb)
		return () => removeListener(this.listeners, cb)
	}

	onError(cb: (error: unknown) => void): () => void {
		this.errorListeners.push(cb)
		return () => removeListener(this.errorListeners, cb)
	}

	onClose(cb: () => void): () => void {
		this.closeListeners.push(cb)
		return () => removeListener(this.closeListeners, cb)
	}

	close(): void {
		this.abortController.abort()
	}

	private async readLoop(): Promise<void> {
		try {
			if (!this.response.body) throw new Error('Core event stream response has no body')
			const reader = this.response.body.getReader()
			this.abortController.signal.addEventListener('abort', () => reader.cancel().catch(() => {}), {
				once: true,
			})
			const decoder = new TextDecoder()
			let buffer = ''

			while (!this.abortController.signal.aborted) {
				const { done, value } = await reader.read()
				if (done) break
				buffer += decoder.decode(value, { stream: true })
				buffer = this.consumeBuffer(buffer)
			}

			buffer += decoder.decode()
			this.consumeBuffer(`${buffer}\n\n`)
		} catch (error) {
			if (!this.abortController.signal.aborted) {
				for (const cb of this.errorListeners) cb(error)
			}
		} finally {
			for (const cb of this.closeListeners) cb()
		}
	}

	private consumeBuffer(buffer: string): string {
		const normalized = buffer.replace(/\r\n/g, '\n')
		const events = normalized.split('\n\n')
		const remainder = events.pop() ?? ''
		for (const event of events) {
			const data = event
				.split('\n')
				.filter((line) => line.startsWith('data:'))
				.map((line) => line.slice(5).trimStart())
				.join('\n')
			if (!data) continue
			try {
				const parsed = JSON.parse(data) as CoreInstanceEvent
				for (const cb of this.listeners) cb(parsed)
			} catch (error) {
				for (const cb of this.errorListeners) cb(error)
			}
		}
		return remainder
	}
}

function removeListener<T>(listeners: T[], listener: T): void {
	const index = listeners.indexOf(listener)
	if (index !== -1) listeners.splice(index, 1)
}
