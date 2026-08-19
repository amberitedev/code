import type { CoreClientAdapter } from './adapter'
import * as api from './api'
import type { CoreCallContext } from './context'
import { CoreOfflineError } from './errors'
import { type ConnectionStatus, CoreConnectionMonitor } from './monitor'
import type {
	CoreAccessPatchBody,
	CoreAccessResponse,
	CoreAccessUpsertBody,
	CoreActivityLogQuery,
	CoreActivityLogResponse,
	CoreBackup,
	CoreBackupSchedule,
	CoreBackupScheduleBody,
	CoreBackupsResponse,
	CoreChangeVersionBody,
	CoreCreateInstanceBody,
	CoreCreateSyncProfileFromMrpackMetadata,
	CoreCreateTaskBody,
	CoreFsListing,
	CoreInstance,
	CoreInstanceEvent,
	CoreInstanceSummary,
	CoreInvitation,
	CoreMember,
	CoreMetadata,
	CoreMod,
	CoreModpackManifest,
	CoreNetworkStatus,
	CorePlayers,
	CoreProjectionSyncResult,
	CorePublishSyncSnapshotMetadata,
	CoreRconEnableResult,
	CoreRole,
	CoreRoleConfiguration,
	CoreScheduledTask,
	CoreServerQuery,
	CoreSetupDevResetResponse,
	CoreSetupRequest,
	CoreSetupResponse,
	CoreSetupStatus,
	CoreStartupSettings,
	CoreStats,
	CoreSyncEvent,
	CoreSyncProfile,
	CoreSyncSnapshot,
	CoreSyncSnapshotPublishResult,
	CoreSyncVersionStatus,
	CoreUpdateTaskBody,
	CoreUploadHandle,
	CoreUploadSession,
	CreateCoreInvitationBody,
	FsCopyRequest,
	FsDownloadUrlResponse,
	FsZipRequest,
	SaveCoreRoleBody,
	UnzipOption,
	UpdateCoreInvitationBody,
} from './types'
import { CoreWsConnection } from './ws'

export interface CoreClientOptions {
	timeoutMs?: number
}

export class CoreApiClient {
	public readonly monitor: CoreConnectionMonitor
	private coreUrl: string | null = null
	private hasCoreUrl = false

	constructor(
		public readonly adapter: CoreClientAdapter,
		private readonly options: CoreClientOptions = {},
	) {
		this.monitor = new CoreConnectionMonitor(adapter)
	}

	private async getCoreUrlCached(): Promise<string | null> {
		const next = await this.adapter.getCoreUrl()
		if (!this.hasCoreUrl || this.coreUrl !== next) {
			this.coreUrl = next
			this.hasCoreUrl = true
		}
		return this.coreUrl
	}

	clearCoreUrlCache(): void {
		this.coreUrl = null
		this.hasCoreUrl = false
	}

	private async createContext(coreUrl?: string, authenticated = true): Promise<CoreCallContext> {
		const resolvedCoreUrl = coreUrl ?? (await this.getCoreUrlCached())
		if (!resolvedCoreUrl) throw new CoreOfflineError()
		if (authenticated) await this.verifyConnectedCore(resolvedCoreUrl)
		return {
			baseUrl: resolvedCoreUrl.replace(/\/$/, ''),
			token: authenticated ? await this.adapter.getCurrentJwt() : null,
			fetchFn: this.adapter.fetchFn,
			timeoutMs: this.options.timeoutMs,
		}
	}

	private async verifyConnectedCore(coreUrl: string): Promise<void> {
		const knownCoreId = await this.adapter.getConnectedCoreId?.()
		if (!knownCoreId) return

		const current = this.monitor.currentStatus
		if (
			current?.state === 'connected' &&
			current.coreUrl === coreUrl &&
			current.coreId === knownCoreId
		) {
			return
		}

		const checked = await this.monitor.checkNow(coreUrl)
		if (checked.state !== 'connected' || checked.coreId !== knownCoreId) {
			throw new CoreOfflineError()
		}
	}

	private async call<T>(fn: (ctx: CoreCallContext) => Promise<T>): Promise<T> {
		return await fn(await this.createContext())
	}

	private upload(createHandle: (ctx: CoreCallContext) => CoreUploadHandle): CoreUploadHandle {
		const progressCallbacks: Array<(percent: number) => void> = []
		let innerAbort = () => {}
		let aborted = false
		const done = (async () => {
			const handle = createHandle(await this.createContext())
			innerAbort = handle.abort
			if (aborted) {
				handle.abort()
				return
			}
			handle.onProgress((percent) => {
				for (const callback of progressCallbacks) callback(percent)
			})
			await handle.done
		})()
		return {
			onProgress: (callback) => progressCallbacks.push(callback),
			done,
			abort: () => {
				aborted = true
				innerAbort()
			},
		}
	}

	getSetupStatus(): Promise<CoreSetupStatus> {
		return this.call(api.getSetupStatus)
	}

	completeSetup(body: CoreSetupRequest): Promise<CoreSetupResponse> {
		return this.call((ctx) => api.completeSetup(ctx, body))
	}

	devResetSetup(): Promise<CoreSetupDevResetResponse> {
		return this.call(api.devResetSetup)
	}

	devResetSetupAt(coreUrl: string): Promise<CoreSetupDevResetResponse> {
		return api.devResetSetup({
			baseUrl: coreUrl.replace(/\/$/, ''),
			token: null,
			fetchFn: this.adapter.fetchFn,
			timeoutMs: this.options.timeoutMs,
		})
	}

	async completeSetupAt(coreUrl: string, body: CoreSetupRequest): Promise<CoreSetupResponse> {
		return await api.completeSetup(await this.createContext(coreUrl, false), body)
	}

	connect(): Promise<ConnectionStatus> {
		return this.monitor.checkNow()
	}

	getNetworkStatus(): Promise<CoreNetworkStatus> {
		return this.call(api.getNetworkStatus)
	}

	// ── Instances ───────────────────────────────────────────────────────────

	listInstances(): Promise<CoreInstanceSummary[]> {
		return this.call((ctx) => api.listInstances(ctx).then((r) => r.instances))
	}

	getInstance(id: string): Promise<CoreInstance> {
		return this.call((ctx) => api.getInstance(ctx, id))
	}

	createInstance(body: CoreCreateInstanceBody): Promise<CoreInstance> {
		return this.call((ctx) => api.createInstance(ctx, body))
	}

	deleteInstance(id: string): Promise<void> {
		return this.call((ctx) => api.deleteInstance(ctx, id).then(() => undefined))
	}

	renameInstance(id: string, name: string): Promise<CoreInstance> {
		return this.call((ctx) => api.patchInstance(ctx, id, { name }))
	}

	updateJavaVersion(id: string, javaVersion: number | null): Promise<CoreInstance> {
		return this.call((ctx) => api.patchInstance(ctx, id, { java_version: javaVersion }))
	}

	/** Set RAM bounds (MB). Applied on next start. */
	updateMemory(id: string, minMb: number, maxMb: number): Promise<CoreInstance> {
		const memory = { min_mb: minMb, max_mb: maxMb }
		return this.call((ctx) => api.patchInstance(ctx, id, { memory }))
	}

	/**
	 * Update the startup overrides. Pass `null` for a field to clear that override
	 * and fall back to Core's defaults; omit a field to leave it unchanged.
	 */
	updateStartup(
		id: string,
		overrides: { jvm_args?: string | null; server_args?: string | null },
	): Promise<CoreInstance> {
		return this.call((ctx) => api.patchInstance(ctx, id, overrides))
	}

	/** Current startup configuration plus rendered default/effective launch commands. */
	getStartup(id: string): Promise<CoreStartupSettings> {
		return this.call((ctx) => api.getStartup(ctx, id))
	}

	// ── Lifecycle ───────────────────────────────────────────────────────────

	start(id: string): Promise<void> {
		return this.call((ctx) => api.startInstance(ctx, id).then(() => undefined))
	}

	stop(id: string): Promise<void> {
		return this.call((ctx) => api.stopInstance(ctx, id).then(() => undefined))
	}

	kill(id: string): Promise<void> {
		return this.call((ctx) => api.killInstance(ctx, id).then(() => undefined))
	}

	restart(id: string): Promise<void> {
		return this.call((ctx) => api.restartInstance(ctx, id).then(() => undefined))
	}

	repair(id: string): Promise<void> {
		return this.call((ctx) => api.repairInstance(ctx, id).then(() => undefined))
	}

	changeVersion(id: string, body: CoreChangeVersionBody): Promise<void> {
		return this.call((ctx) => api.changeInstanceVersion(ctx, id, body).then(() => undefined))
	}

	sendCommand(id: string, command: string): Promise<void> {
		return this.call((ctx) => api.sendCommand(ctx, id, command).then(() => undefined))
	}

	// ── WebSocket ───────────────────────────────────────────────────────────

	async issueWsTicket(): Promise<string> {
		return this.call((ctx) => api.issueWsTicket(ctx).then((r) => r.ticket))
	}

	openEvents(): Promise<CoreEventStream> {
		return this.call(async (ctx) => {
			const response = await api.openEventStream(ctx)
			return new CoreEventStream(response)
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
		return this.call((ctx) => api.getStats(ctx, id))
	}

	// ── RCON ────────────────────────────────────────────────────────────────

	/** Execute a raw RCON command against a running instance. */
	executeRcon(id: string, command: string): Promise<{ response: string }> {
		return this.call((ctx) => api.executeRcon(ctx, id, command))
	}

	/** Enable RCON on an instance (patches server.properties, may need restart). */
	enableRcon(id: string): Promise<CoreRconEnableResult> {
		return this.call((ctx) => api.enableRcon(ctx, id))
	}

	/** Query a running instance via Server List Ping (MOTD, version, players, latency). */
	queryInstance(id: string): Promise<CoreServerQuery> {
		return this.call((ctx) => api.queryInstance(ctx, id))
	}

	// ── Scheduled Tasks ─────────────────────────────────────────────────────

	listTasks(id: string): Promise<CoreScheduledTask[]> {
		return this.call((ctx) => api.listTasks(ctx, id).then((r) => r.tasks))
	}

	createTask(id: string, body: CoreCreateTaskBody): Promise<CoreScheduledTask> {
		return this.call((ctx) => api.createTask(ctx, id, body))
	}

	getTask(id: string, taskId: string): Promise<CoreScheduledTask> {
		return this.call((ctx) => api.getTask(ctx, id, taskId))
	}

	updateTask(id: string, taskId: string, body: CoreUpdateTaskBody): Promise<CoreScheduledTask> {
		return this.call((ctx) => api.updateTask(ctx, id, taskId, body))
	}

	deleteTask(id: string, taskId: string): Promise<void> {
		return this.call((ctx) => api.deleteTask(ctx, id, taskId).then(() => undefined))
	}

	// ── Players ─────────────────────────────────────────────────────────────

	/** Full player-management snapshot (online list + whitelist/ops/bans). */
	listPlayers(id: string): Promise<CorePlayers> {
		return this.call((ctx) => api.listPlayers(ctx, id))
	}

	kickPlayer(id: string, name: string, reason?: string): Promise<{ response: string }> {
		return this.call((ctx) => api.kickPlayer(ctx, id, name, reason))
	}

	opPlayer(id: string, name: string): Promise<{ response: string }> {
		return this.call((ctx) => api.opPlayer(ctx, id, name))
	}

	deopPlayer(id: string, name: string): Promise<{ response: string }> {
		return this.call((ctx) => api.deopPlayer(ctx, id, name))
	}

	banPlayer(id: string, name: string, reason?: string): Promise<{ response: string }> {
		return this.call((ctx) => api.banPlayer(ctx, id, name, reason))
	}

	pardonPlayer(id: string, name: string): Promise<{ response: string }> {
		return this.call((ctx) => api.pardonPlayer(ctx, id, name))
	}

	banIp(id: string, ip: string, reason?: string): Promise<{ response: string }> {
		return this.call((ctx) => api.banIp(ctx, id, ip, reason))
	}

	pardonIp(id: string, ip: string): Promise<{ response: string }> {
		return this.call((ctx) => api.pardonIp(ctx, id, ip))
	}

	addToWhitelist(id: string, name: string): Promise<{ response: string }> {
		return this.call((ctx) => api.addToWhitelist(ctx, id, name))
	}

	removeFromWhitelist(id: string, name: string): Promise<{ response: string }> {
		return this.call((ctx) => api.removeFromWhitelist(ctx, id, name))
	}

	setWhitelistEnabled(id: string, enabled: boolean): Promise<{ response: string }> {
		return this.call((ctx) => api.setWhitelistEnabled(ctx, id, enabled))
	}

	// ── Mods ────────────────────────────────────────────────────────────────
	listMods(id: string): Promise<CoreMod[]> {
		return this.call((ctx) => api.listMods(ctx, id).then((r) => r.mods))
	}

	addMod(id: string, versionId: string): Promise<CoreMod> {
		return this.call((ctx) => api.addMod(ctx, id, versionId))
	}

	addModProject(id: string, projectId: string): Promise<CoreMod> {
		return this.call((ctx) => api.addModProject(ctx, id, projectId))
	}

	installModpackVersion(
		id: string,
		projectId: string,
		versionId: string,
	): Promise<CoreModpackManifest> {
		return this.call((ctx) => api.installModpackVersion(ctx, id, projectId, versionId))
	}

	uploadModFile(id: string, file: File): CoreUploadHandle {
		return this.upload((ctx) => api.uploadModFile(ctx, id, file))
	}

	deleteMod(id: string, filename: string): Promise<{ restart_required: boolean }> {
		return this.call((ctx) => api.deleteMod(ctx, id, filename))
	}

	toggleMod(id: string, filename: string, enabled: boolean): Promise<void> {
		return this.call((ctx) => api.toggleMod(ctx, id, filename, enabled).then(() => undefined))
	}

	updateMod(id: string, filename: string): Promise<boolean> {
		return this.call((ctx) => api.updateMod(ctx, id, filename).then((r) => r.updated))
	}

	updateAllMods(
		id: string,
	): Promise<{ updated: string[]; already_latest: string[]; failed: unknown[] }> {
		return this.call((ctx) => api.updateAllMods(ctx, id))
	}

	// ── Logs ────────────────────────────────────────────────────────────────

	listLogs(id: string): Promise<string[]> {
		return this.call((ctx) => api.listLogs(ctx, id).then((r) => r.logs))
	}

	readLog(id: string, filename: string): Promise<string> {
		return this.call((ctx) => api.readLog(ctx, id, filename))
	}

	listCrashReports(id: string): Promise<string[]> {
		return this.call((ctx) => api.listCrashReports(ctx, id).then((r) => r.crash_reports))
	}

	readCrashReport(id: string, filename: string): Promise<string> {
		return this.call((ctx) => api.readCrashReport(ctx, id, filename))
	}

	// ── Server properties ───────────────────────────────────────────────────

	getProperties(id: string): Promise<Record<string, string>> {
		return this.call((ctx) => api.getProperties(ctx, id).then((r) => r.properties))
	}

	patchProperties(id: string, updates: Record<string, string>): Promise<void> {
		return this.call((ctx) => api.patchProperties(ctx, id, updates).then(() => undefined))
	}

	// ── Filesystem ──────────────────────────────────────────────────────────

	listDirectory(id: string, path: string, page = 0, pageSize = 50): Promise<CoreFsListing> {
		return this.call((ctx) => api.listDirectory(ctx, id, path, page, pageSize))
	}

	downloadFile(id: string, path: string): Promise<Blob> {
		return this.call((ctx) => api.downloadFile(ctx, id, path))
	}

	deleteFileOrFolder(id: string, path: string, recursive = false): Promise<void> {
		return this.call((ctx) =>
			api.deleteFileOrFolder(ctx, id, path, recursive).then(() => undefined),
		)
	}

	uploadFile(id: string, targetDir: string, file: File): CoreUploadHandle {
		return this.upload((ctx) => api.uploadFile(ctx, id, targetDir, file))
	}

	createResumableUpload(
		id: string,
		path: string,
		length: number,
		sha256Hex?: string,
	): Promise<CoreUploadSession> {
		return this.call((ctx) => api.createResumableUpload(ctx, id, path, length, sha256Hex))
	}

	getResumableUploadStatus(id: string, uploadId: string): Promise<CoreUploadSession> {
		return this.call((ctx) => api.getResumableUploadStatus(ctx, id, uploadId))
	}

	appendResumableUpload(
		id: string,
		uploadId: string,
		offset: number,
		chunk: BodyInit,
		sha256DigestBase64?: string,
	): Promise<CoreUploadSession> {
		return this.call((ctx) =>
			api.appendResumableUpload(ctx, id, uploadId, offset, chunk, sha256DigestBase64),
		)
	}

	cancelResumableUpload(id: string, uploadId: string): Promise<void> {
		return this.call((ctx) => api.cancelResumableUpload(ctx, id, uploadId).then(() => undefined))
	}

	readFile(id: string, path: string): Promise<ArrayBuffer> {
		return this.call((ctx) => api.readFile(ctx, id, path))
	}

	writeFile(id: string, path: string, content: string | ArrayBuffer): Promise<void> {
		return this.call((ctx) => api.writeFile(ctx, id, path, content).then(() => undefined))
	}

	createFile(id: string, path: string): Promise<void> {
		return this.call((ctx) => api.createFile(ctx, id, path).then(() => undefined))
	}

	createDir(id: string, path: string): Promise<void> {
		return this.call((ctx) => api.createDir(ctx, id, path).then(() => undefined))
	}

	moveEntry(id: string, from: string, to: string): Promise<void> {
		return this.call((ctx) => api.moveEntry(ctx, id, from, to).then(() => undefined))
	}

	unzipFile(id: string, path: string, option: UnzipOption = 'normal'): Promise<void> {
		return this.call((ctx) => api.unzipFile(ctx, id, path, option).then(() => undefined))
	}

	zipFiles(id: string, req: FsZipRequest): Promise<void> {
		return this.call((ctx) => api.zipFiles(ctx, id, req).then(() => undefined))
	}

	copyFiles(id: string, req: FsCopyRequest): Promise<void> {
		return this.call((ctx) => api.copyFiles(ctx, id, req).then(() => undefined))
	}

	getDownloadUrl(id: string, path: string): Promise<FsDownloadUrlResponse> {
		return this.call((ctx) => api.getDownloadUrl(ctx, id, path))
	}

	searchFiles(
		id: string,
		path: string,
		query: string,
		recursive = false,
	): Promise<import('./types').CoreFsEntry[]> {
		return this.call((ctx) => api.searchFiles(ctx, id, path, query, recursive))
	}

	// ── Backups ─────────────────────────────────────────────────────────────

	listBackups(id: string): Promise<CoreBackupsResponse> {
		return this.call((ctx) => api.listBackups(ctx, id))
	}

	createBackup(id: string, name?: string): Promise<CoreBackup> {
		return this.call((ctx) => api.createBackup(ctx, id, name))
	}

	renameBackup(id: string, backupId: string, name: string): Promise<void> {
		return this.call((ctx) => api.renameBackup(ctx, id, backupId, name).then(() => undefined))
	}

	deleteBackup(id: string, backupId: string): Promise<void> {
		return this.call((ctx) => api.deleteBackup(ctx, id, backupId).then(() => undefined))
	}

	deleteManyBackups(id: string, backupIds: string[]): Promise<number> {
		return this.call((ctx) => api.deleteManyBackups(ctx, id, backupIds).then((r) => r.deleted))
	}

	lockBackup(id: string, backupId: string, locked: boolean): Promise<void> {
		return this.call((ctx) => api.lockBackup(ctx, id, backupId, locked).then(() => undefined))
	}

	restoreBackup(id: string, backupId: string): Promise<void> {
		return this.call((ctx) => api.restoreBackup(ctx, id, backupId).then(() => undefined))
	}

	getBackupSchedule(id: string): Promise<CoreBackupSchedule> {
		return this.call((ctx) => api.getBackupSchedule(ctx, id))
	}

	setBackupSchedule(id: string, schedule: CoreBackupScheduleBody): Promise<void> {
		return this.call((ctx) => api.setBackupSchedule(ctx, id, schedule).then(() => undefined))
	}

	// ── Social / Core Identity ───────────────────────────────────────────────

	getCoreMetadata(): Promise<CoreMetadata> {
		return this.call(api.getCoreMetadata)
	}

	updateCoreMetadata(body: Partial<CoreMetadata>): Promise<CoreMetadata> {
		return this.call((ctx) => api.updateCoreMetadata(ctx, body))
	}

	listCoreMembers(): Promise<CoreMember[]> {
		return this.call((ctx) => api.listCoreMembers(ctx).then((r) => r.members))
	}

	upsertCoreMember(body: Partial<CoreMember> & { user_id: string }): Promise<CoreMember> {
		return this.call((ctx) => api.upsertCoreMember(ctx, body))
	}

	removeCoreMember(userId: string): Promise<void> {
		return this.call((ctx) => api.removeCoreMember(ctx, userId).then(() => undefined))
	}

	resyncCoreProjection(): Promise<CoreProjectionSyncResult> {
		return this.call(api.resyncCoreProjection)
	}

	listCoreAccess(): Promise<CoreAccessResponse> {
		return this.call(api.listCoreAccess)
	}

	grantCoreAccess(body: CoreAccessUpsertBody): Promise<CoreMember> {
		return this.call((ctx) => api.grantCoreAccess(ctx, body))
	}

	updateCoreAccess(userId: string, body: CoreAccessPatchBody): Promise<CoreMember> {
		return this.call((ctx) => api.updateCoreAccess(ctx, userId, body))
	}

	removeCoreAccess(userId: string): Promise<void> {
		return this.call((ctx) => api.removeCoreAccess(ctx, userId).then(() => undefined))
	}

	getCoreRoles(): Promise<CoreRoleConfiguration> {
		return this.call(api.getCoreRoles)
	}

	saveCoreRole(body: SaveCoreRoleBody): Promise<CoreRole> {
		return this.call((ctx) => api.saveCoreRole(ctx, body))
	}

	retireCoreRole(id: string): Promise<void> {
		return this.call((ctx) => api.retireCoreRole(ctx, id).then(() => undefined))
	}

	createCoreInvitation(body: CreateCoreInvitationBody): Promise<CoreInvitation> {
		return this.call((ctx) => api.createCoreInvitation(ctx, body))
	}
	updateCoreInvitation(id: string, body: UpdateCoreInvitationBody): Promise<CoreInvitation> {
		return this.call((ctx) => api.updateCoreInvitation(ctx, id, body))
	}
	listCoreInvitations(): Promise<CoreInvitation[]> {
		return this.call((ctx) => api.listCoreInvitations(ctx).then((result) => result.invitations))
	}
	listMyCoreInvitations(): Promise<CoreInvitation[]> {
		return this.call((ctx) => api.listMyCoreInvitations(ctx).then((result) => result.invitations))
	}
	reviewCoreInvitation(id: string, accept: boolean): Promise<CoreInvitation> {
		return this.call((ctx) => api.reviewCoreInvitation(ctx, id, accept))
	}
	revokeCoreInvitation(id: string): Promise<CoreInvitation> {
		return this.call((ctx) => api.revokeCoreInvitation(ctx, id))
	}
	respondToCoreInvitation(id: string, accept: boolean): Promise<CoreInvitation> {
		return this.call((ctx) => api.respondToCoreInvitation(ctx, id, accept))
	}

	listInstanceAccess(id: string): Promise<CoreAccessResponse> {
		return this.call((ctx) => api.listInstanceAccess(ctx, id))
	}

	grantInstanceAccess(id: string, body: CoreAccessUpsertBody): Promise<void> {
		return this.call((ctx) => api.grantInstanceAccess(ctx, id, body).then(() => undefined))
	}

	updateInstanceAccess(id: string, userId: string, body: CoreAccessPatchBody): Promise<void> {
		return this.call((ctx) => api.updateInstanceAccess(ctx, id, userId, body).then(() => undefined))
	}

	removeInstanceAccess(id: string, userId: string): Promise<void> {
		return this.call((ctx) => api.removeInstanceAccess(ctx, id, userId).then(() => undefined))
	}

	listActivity(query?: CoreActivityLogQuery): Promise<CoreActivityLogResponse> {
		return this.call((ctx) => api.listActivity(ctx, query))
	}

	listInstanceActivity(id: string, query?: CoreActivityLogQuery): Promise<CoreActivityLogResponse> {
		return this.call((ctx) => api.listInstanceActivity(ctx, id, query))
	}

	// ── Sync Profiles ────────────────────────────────────────────────────────

	listSyncProfiles(): Promise<CoreSyncProfile[]> {
		return this.call((ctx) => api.listSyncProfiles(ctx).then((r) => r.profiles))
	}

	registerSyncProfile(body: Partial<CoreSyncProfile> & { name: string }): Promise<CoreSyncProfile> {
		return this.call((ctx) => api.registerSyncProfile(ctx, body))
	}

	removeSyncProfile(profileId: string): Promise<void> {
		return this.call((ctx) => api.removeSyncProfile(ctx, profileId).then(() => undefined))
	}

	createSyncProfileFromMrpack(
		file: File,
		metadata?: CoreCreateSyncProfileFromMrpackMetadata,
	): Promise<CoreSyncSnapshotPublishResult> {
		return this.call((ctx) => api.createSyncProfileFromMrpack(ctx, file, metadata))
	}

	publishSyncSnapshot(
		profileId: string,
		file: File,
		metadata?: string | CorePublishSyncSnapshotMetadata,
	): Promise<CoreSyncSnapshotPublishResult> {
		return this.call((ctx) => api.publishSyncSnapshot(ctx, profileId, file, metadata))
	}

	listSyncSnapshots(profileId: string): Promise<CoreSyncSnapshot[]> {
		return this.call((ctx) => api.listSyncSnapshots(ctx, profileId).then((r) => r.snapshots))
	}

	listSyncEvents(profileId: string): Promise<CoreSyncEvent[]> {
		return this.call((ctx) => api.listSyncEvents(ctx, profileId).then((r) => r.events))
	}

	checkSyncVersion(profileId: string): Promise<CoreSyncVersionStatus> {
		return this.call((ctx) => api.checkSyncVersion(ctx, profileId))
	}

	downloadSyncSnapshot(profileId: string, snapshotId: string): Promise<Blob> {
		return this.call((ctx) => api.downloadSyncSnapshot(ctx, profileId, snapshotId))
	}

	// ── Modpack (additional) ─────────────────────────────────────────────────

	getModpack(id: string): Promise<CoreModpackManifest | null> {
		return this.call((ctx) => api.getModpack(ctx, id))
	}

	removeModpack(id: string): Promise<void> {
		return this.call((ctx) => api.removeModpack(ctx, id).then(() => undefined))
	}

	exportModpack(id: string): Promise<Blob> {
		return this.call((ctx) => api.exportModpack(ctx, id))
	}

	installModpackFile(id: string, file: File): Promise<CoreModpackManifest> {
		return this.call((ctx) => api.installModpackFile(ctx, id, file))
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
