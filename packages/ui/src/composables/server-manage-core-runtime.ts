import type { CoreInstanceStatus, CoreStats, CoreWsConnection } from '@amberite/core-client'
import type { Archon, UploadState } from '@modrinth/api-client'
import type { Stats } from '@modrinth/utils'
import type { ComputedRef, Ref } from 'vue'
import { computed, ref } from 'vue'

import type { FileOperation } from '../layouts/shared/files-tab/types'
import { injectCoreClient, provideModrinthServerContext } from '../providers'
import type { BusyReason } from '../providers/server-context'
import { defineMessage } from './i18n'
import { useModrinthServersConsole } from './server-console'

type ReadableRef<T> = Ref<T> | ComputedRef<T>

type UseServerManageCoreRuntimeOptions = {
	serverId: ReadableRef<string>
	worldId: ReadableRef<string | null>
	server: ReadableRef<Archon.Servers.v0.Server | null | undefined>
	isSyncingContent: ReadableRef<boolean>
	extraBusyReasons?: ComputedRef<BusyReason[]>
	incrementUptimeLocally?: boolean
	eventGuard?: () => boolean
}

const STALE_MS = 5000
const STALE_INTERVAL_MS = 1000

const mapPowerState = (s: CoreInstanceStatus): Archon.Websocket.v0.PowerState =>
	({ offline: 'stopped', starting: 'starting', running: 'running', stopping: 'stopping', crashed: 'crashed' } as const)[s]

const mapCoreStats = (s: CoreStats): Stats['current'] => ({
	cpu_percent: s.cpu_percent ?? 0,
	ram_usage_bytes: (s.memory_mb ?? 0) * 1024 * 1024,
	ram_total_bytes: (s.ram_total_mb ?? 1) * 1024 * 1024,
	storage_usage_bytes: 0,
	storage_total_bytes: 0,
})

const emptyStats = (): Stats => ({
	current: { cpu_percent: 0, ram_usage_bytes: 0, ram_total_bytes: 1, storage_usage_bytes: 0, storage_total_bytes: 0 },
	past: { cpu_percent: 0, ram_usage_bytes: 0, ram_total_bytes: 1, storage_usage_bytes: 0, storage_total_bytes: 0 },
	graph: { cpu: [], ram: [] },
})

const appendGraph = (arr: number[], v: number): number[] => {
	const next = [...arr, v]
	if (next.length > 10) next.shift()
	return next
}

export function useServerManageCoreRuntime(options: UseServerManageCoreRuntimeOptions) {
	const coreClient = injectCoreClient()
	const console$ = useModrinthServersConsole()
	const guard = () => (options.eventGuard ? options.eventGuard() : true)

	const isConnected = ref(false)
	const isWsAuthIncorrect = ref(false)
	const serverPowerState = ref<Archon.Websocket.v0.PowerState>('stopped')
	const powerStateDetails = ref<{ oom_killed?: boolean; exit_code?: number }>()
	const isServerRunning = computed(() => serverPowerState.value === 'running')
	const stats = ref<Stats>(emptyStats())
	const uptimeSeconds = ref(0)
	const cpuData = ref<number[]>([])
	const ramData = ref<number[]>([])

	const fsAuth = ref<{ url: string; token: string } | null>(null)
	const fsOps = ref<Archon.Websocket.v0.FilesystemOperation[]>([])
	const fsQueuedOps = ref<Archon.Websocket.v0.QueuedFilesystemOp[]>([])
	const activeOperations = computed<FileOperation[]>(() => [])
	const uploadState = ref<UploadState>({
		isUploading: false, currentFileName: null, currentFileProgress: 0,
		uploadedBytes: 0, totalBytes: 0, completedFiles: 0, totalFiles: 0,
	})
	const cancelUpload = ref<(() => void) | null>(null)

	let wsConn: CoreWsConnection | null = null
	let connectedServerId: string | null = null
	let uptimerId: ReturnType<typeof setInterval> | null = null
	let staleTimeout: ReturnType<typeof setTimeout> | null = null
	let staleInterval: ReturnType<typeof setInterval> | null = null

	const busyReasons = computed<BusyReason[]>(() => {
		const r: BusyReason[] = []
		if (options.server.value?.status === 'installing')
			r.push({ reason: defineMessage({ id: 'servers.busy.installing', defaultMessage: 'Server is installing' }) })
		if (options.isSyncingContent.value)
			r.push({ reason: defineMessage({ id: 'servers.busy.syncing-content', defaultMessage: 'Content sync in progress' }) })
		if (options.extraBusyReasons) r.push(...options.extraBusyReasons.value)
		return r
	})

	const stopUptime = () => { if (uptimerId) { clearInterval(uptimerId); uptimerId = null } }
	const startUptime = () => {
		if (!options.incrementUptimeLocally || uptimerId) return
		uptimerId = setInterval(() => { uptimeSeconds.value += 1 }, 1000)
	}
	const clearStale = () => {
		if (staleTimeout) { clearTimeout(staleTimeout); staleTimeout = null }
		if (staleInterval) { clearInterval(staleInterval); staleInterval = null }
	}
	const pushZero = () => {
		if (!guard()) return
		cpuData.value = appendGraph(cpuData.value, 0)
		ramData.value = appendGraph(ramData.value, 0)
		stats.value = { current: { ...stats.value.current, cpu_percent: 0, ram_usage_bytes: 0 }, past: { ...stats.value.current }, graph: { cpu: cpuData.value, ram: ramData.value } }
	}
	const armStale = () => {
		clearStale()
		staleTimeout = setTimeout(() => { pushZero(); staleInterval = setInterval(pushZero, STALE_INTERVAL_MS) }, STALE_MS)
	}

	const onLog = (msg: string) => { if (guard()) console$.addLegacyLog(msg) }
	const onStats = (s: CoreStats) => {
		if (!guard()) return
		armStale()
		const current = mapCoreStats(s)
		cpuData.value = appendGraph(cpuData.value, current.cpu_percent)
		ramData.value = appendGraph(ramData.value, Math.floor((current.ram_usage_bytes / current.ram_total_bytes) * 100))
		stats.value = { current, past: { ...stats.value.current }, graph: { cpu: cpuData.value, ram: ramData.value } }
		if (!isConnected.value) isConnected.value = true
		if (s.uptime_seconds != null) { stopUptime(); uptimeSeconds.value = s.uptime_seconds; if (serverPowerState.value === 'running') startUptime() }
	}
	const onState = (status: CoreInstanceStatus) => {
		if (!guard()) return
		const ps = mapPowerState(status)
		serverPowerState.value = ps
		if (ps === 'stopped' || ps === 'crashed') { stopUptime(); uptimeSeconds.value = 0; powerStateDetails.value = undefined }
		if (!isConnected.value) isConnected.value = true
	}

	const disconnectSocket = (targetServerId?: string) => {
		if (wsConn) { wsConn.close(); wsConn = null }
		if (targetServerId) connectedServerId = null
		stopUptime(); clearStale()
		isConnected.value = false
		isWsAuthIncorrect.value = false
		serverPowerState.value = 'stopped'
		powerStateDetails.value = undefined
		uptimeSeconds.value = 0
	}

	const connectSocket = async (targetServerId: string, opts: { force?: boolean } = {}): Promise<boolean> => {
		if (connectedServerId === targetServerId && isConnected.value && !opts.force) return true
		disconnectSocket(connectedServerId ?? undefined)
		try {
			const ticket = await coreClient.issueWsTicket()
			console$.clear()
			const conn = coreClient.openConsole(targetServerId, ticket)
			wsConn = conn; connectedServerId = targetServerId
			conn.on('open', () => { isConnected.value = true })
			conn.on('close', () => { isConnected.value = false; clearStale() })
			conn.on('log', onLog)
			conn.on('stats', onStats)
			conn.on('state', onState)
			return true
		} catch (err) {
			console.error('[hosting/manage] Failed to connect server socket:', err)
			isConnected.value = false
			return false
		}
	}

	const dismissOperation = async (_opId: string, _action: 'dismiss' | 'cancel') => {}
	const refreshFsAuth = async () => {}

	provideModrinthServerContext({
		get serverId() { return options.serverId.value },
		worldId: options.worldId as Ref<string | null>,
		server: options.server as Ref<Archon.Servers.v0.Server>,
		isConnected, isWsAuthIncorrect, powerState: serverPowerState, powerStateDetails,
		isServerRunning, stats, uptimeSeconds,
		isSyncingContent: options.isSyncingContent as Ref<boolean>,
		busyReasons, fsAuth, fsOps, fsQueuedOps, refreshFsAuth,
		uploadState, cancelUpload, activeOperations, dismissOperation,
	})

	const cleanupCoreRuntime = (targetServerId?: string) =>
		disconnectSocket(targetServerId ?? connectedServerId ?? undefined)

	return {
		activeOperations, busyReasons, cancelUpload, cleanupCoreRuntime, connectSocket,
		cpuData, disconnectSocket, dismissOperation, fsAuth, fsOps, fsQueuedOps,
		isConnected, isServerRunning, isWsAuthIncorrect, powerStateDetails, ramData,
		refreshFsAuth, serverPowerState, stats, uptimeSeconds, uploadState,
	}
}
