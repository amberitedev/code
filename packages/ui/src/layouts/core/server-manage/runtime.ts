import type { CoreInstanceStatus, CoreStats, CoreWsConnection } from '@amberite/amberite-api'
import type { Archon, UploadState } from '@modrinth/api-client'
import type { Stats } from '@modrinth/utils'
import type { ComputedRef, Ref } from 'vue'
import { computed, ref } from 'vue'

import { useModrinthServersConsole } from '#ui/composables/server-console'
import type { FileOperation } from '#ui/layouts/shared/files-tab/types'
import { injectCoreClient, provideModrinthServerContext } from '#ui/providers'
import type { BusyReason, CancelUploadHandler } from '#ui/providers/server-context'

type ReadableRef<T> = Ref<T> | ComputedRef<T>

type Options = {
	serverId: ReadableRef<string>
	worldId?: ReadableRef<string | null>
	server: ReadableRef<Archon.Servers.v0.Server | null | undefined>
	isSyncingContent?: ReadableRef<boolean>
	extraBusyReasons?: ComputedRef<BusyReason[]>
	incrementUptimeLocally?: boolean
	eventGuard?: () => boolean
}

const emptyStats = (): Stats => ({
	current: {
		cpu_percent: 0,
		ram_usage_bytes: 0,
		ram_total_bytes: 1,
		storage_usage_bytes: 0,
		storage_total_bytes: 0,
	},
	past: {
		cpu_percent: 0,
		ram_usage_bytes: 0,
		ram_total_bytes: 1,
		storage_usage_bytes: 0,
		storage_total_bytes: 0,
	},
	graph: { cpu: [], ram: [] },
})

const mapPowerState = (status: CoreInstanceStatus): Archon.Websocket.v0.PowerState =>
	({
		offline: 'stopped',
		starting: 'starting',
		running: 'running',
		stopping: 'stopping',
		crashed: 'crashed',
	})[status]

const mapCoreStats = (stats: CoreStats): Stats['current'] => ({
	cpu_percent: stats.cpu_percent ?? 0,
	ram_usage_bytes: (stats.memory_mb ?? 0) * 1024 * 1024,
	ram_total_bytes: (stats.ram_total_mb ?? 1) * 1024 * 1024,
	storage_usage_bytes: 0,
	storage_total_bytes: 0,
})

const appendGraph = (values: number[], value: number) => [...values.slice(-9), value]

export function useCoreServerManageRuntime(options: Options) {
	const coreClient = injectCoreClient()
	const consoleState = useModrinthServersConsole()
	const guard = () => options.eventGuard?.() ?? true

	const isConnected = ref(false)
	const isConnecting = ref(false)
	const isReconnecting = ref(false)
	const isWsAuthIncorrect = ref(false)
	const powerState = ref<Archon.Websocket.v0.PowerState>('stopped')
	const powerStateDetails = ref<{ oom_killed?: boolean; exit_code?: number }>()
	const isServerRunning = computed(() => powerState.value === 'running')
	const stats = ref<Stats>(emptyStats())
	const uptimeSeconds = ref(0)
	const cpuData = ref<number[]>([])
	const ramData = ref<number[]>([])
	const worldId = options.worldId ?? ref(null)
	const isSyncingContent = options.isSyncingContent ?? ref(false)
	const busyReasons = computed(() => options.extraBusyReasons?.value ?? [])
	const fsAuth = ref(null)
	const fsOps = ref<Archon.Websocket.v0.FilesystemOperation[]>([])
	const fsQueuedOps = ref<Archon.Websocket.v0.QueuedFilesystemOp[]>([])
	const activeOperations = computed<FileOperation[]>(() => [])
	const uploadState = ref<UploadState>({
		isUploading: false,
		currentFileName: null,
		currentFileProgress: 0,
		uploadedBytes: 0,
		totalBytes: 0,
		completedFiles: 0,
		totalFiles: 0,
	})
	const cancelUpload = ref<CancelUploadHandler | null>(null)

	let wsConn: CoreWsConnection | null = null
	let connectedServerId: string | null = null
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null
	let uptimeTimer: ReturnType<typeof setInterval> | null = null
	let shouldReconnect = false

	const stopUptime = () => {
		if (uptimeTimer) clearInterval(uptimeTimer)
		uptimeTimer = null
	}

	const startUptime = () => {
		if (!options.incrementUptimeLocally || uptimeTimer) return
		uptimeTimer = setInterval(() => (uptimeSeconds.value += 1), 1000)
	}

	const disconnectSocket = () => {
		shouldReconnect = false
		if (reconnectTimer) clearTimeout(reconnectTimer)
		reconnectTimer = null
		wsConn?.close()
		wsConn = null
		connectedServerId = null
		isConnected.value = false
		isConnecting.value = false
		isReconnecting.value = false
		isWsAuthIncorrect.value = false
		powerState.value = 'stopped'
		powerStateDetails.value = undefined
		stopUptime()
	}

	const connectSocket = async (targetServerId: string): Promise<boolean> => {
		if (!targetServerId) return false
		if (connectedServerId === targetServerId && isConnected.value) return true
		disconnectSocket()
		shouldReconnect = true

		try {
			isConnecting.value = true
			const ticket = await coreClient.issueWsTicket()
			consoleState.clear()
			wsConn = await coreClient.openConsole(targetServerId, ticket)
			connectedServerId = targetServerId
			wsConn.on('open', () => {
				isConnected.value = true
				isConnecting.value = false
				isReconnecting.value = false
			})
			wsConn.on('close', () => {
				isConnected.value = false
				isConnecting.value = false
				if (shouldReconnect) {
					isReconnecting.value = true
					reconnectTimer = setTimeout(() => void connectSocket(targetServerId), 3000)
				}
			})
			wsConn.on('log', (message) => {
				if (guard()) consoleState.addLegacyLog(message)
			})
			wsConn.on('state', (state) => {
				if (!guard()) return
				powerState.value = mapPowerState(state)
				if (powerState.value === 'running') startUptime()
				else stopUptime()
			})
			wsConn.on('stats', (nextStats) => {
				if (!guard()) return
				const current = mapCoreStats(nextStats)
				cpuData.value = appendGraph(cpuData.value, current.cpu_percent ?? 0)
				ramData.value = appendGraph(
					ramData.value,
					Math.floor((current.ram_usage_bytes / current.ram_total_bytes) * 100),
				)
				stats.value = {
					current,
					past: stats.value.current,
					graph: { cpu: cpuData.value, ram: ramData.value },
				}
				if (nextStats.uptime_seconds != null) uptimeSeconds.value = nextStats.uptime_seconds
			})
			return true
		} catch (error) {
			console.error('[core/server-manage] Failed to connect socket:', error)
			isConnected.value = false
			isConnecting.value = false
			isReconnecting.value = shouldReconnect
			return false
		}
	}

	provideModrinthServerContext({
		get serverId() {
			return options.serverId.value
		},
		worldId: worldId as Ref<string | null>,
		server: options.server as Ref<Archon.Servers.v0.Server>,
		isConnected,
		isWsAuthIncorrect,
		powerState,
		powerStateDetails,
		isServerRunning,
		stats,
		uptimeSeconds,
		isSyncingContent: isSyncingContent as Ref<boolean>,
		busyReasons,
		fsAuth,
		fsOps,
		fsQueuedOps,
		refreshFsAuth: async () => {},
		uploadState,
		cancelUpload,
		activeOperations,
		dismissOperation: async () => {},
	})

	return {
		cleanupCoreRuntime: disconnectSocket,
		connectSocket,
		isServerRunning,
		isConnected,
		isConnecting,
		isReconnecting,
		powerState,
		stats,
		uptimeSeconds,
	}
}
