import {
	CoreApiError,
	CoreOfflineError,
	type CoreChangeVersionBody,
	type CoreInstance,
	type CoreStats,
	type CoreWsConnection,
} from '@amberite/amberite-api'
import type { Archon, UploadState } from '@modrinth/api-client'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, isRef, onMounted, onUnmounted, provide, ref, type Ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import { useModrinthServersConsole } from '#ui/composables/server-console'
import type { FileOperation } from '#ui/layouts/shared/files-tab/types'
import {
	injectHostingBackend,
	injectNotificationManager,
	provideModrinthServerContext,
} from '#ui/providers'
import type { BusyReason, CancelUploadHandler, ServerStats } from '#ui/providers/server-context'

import { coreServerContextKey } from './context'
import {
	appendSocketListeners,
	createFallbackInstance,
	toHostingPowerState,
	toHostingServer,
	toLogLine,
	toStats,
} from './mappers'

type ServerIdInput = string | Ref<string | null | undefined>

export function useCoreServerRuntime(serverIdOverride?: ServerIdInput) {
	const route = useRoute()
	const backend = injectHostingBackend()
	const core = backend.core
	const queryClient = useQueryClient()
	const { addNotification, handleError } = injectNotificationManager()
	const consoleState = useModrinthServersConsole()

	const instanceId = computed(() => {
		if (isRef(serverIdOverride)) return serverIdOverride.value ?? (route.params.id as string)
		return serverIdOverride ?? (route.params.id as string)
	})
	const rawInstance = ref<CoreInstance | null>(null)
	const server = computed(() => (rawInstance.value ? toHostingServer(rawInstance.value) : null))
	const loadError = ref<Error | null>(null)
	const statsData = ref<CoreStats | null>(null)
	const stats = ref<ServerStats>(toStats(null))
	const uptimeSeconds = ref(0)
	const powerState = ref<Archon.Websocket.v0.PowerState>('stopped')
	const isConnected = ref(false)
	const isWsAuthIncorrect = ref(false)
	const isSyncingContent = ref(false)
	const fsAuth = ref<{ url: string; token: string } | null>(null)
	const logLines = consoleState.output
	const cpuHistory = ref<number[]>([])
	const ramHistory = ref<number[]>([])
	const MAX_LOG_LINES = 5000
	let socket: CoreWsConnection | null = null
	let socketListeners: Array<() => void> = []
	let unsubscribeCoreStatus: (() => void) | undefined

	const isServerRunning = computed(() => powerState.value === 'running')
	const serverContextServer = ref<Archon.Servers.v0.Server>(
		toHostingServer(createFallbackInstance(instanceId.value)),
	)
	const worldId = ref<string | null>('default')
	const powerStateDetails = ref<{ oom_killed?: boolean; exit_code?: number } | undefined>(undefined)
	const fsOps = ref<Archon.Websocket.v0.FilesystemOperation[]>([])
	const fsQueuedOps = ref<Archon.Websocket.v0.QueuedFilesystemOp[]>([])
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
	const activeOperations = computed<FileOperation[]>(() => [])
	const busyReasons = computed<BusyReason[]>(() => [])

	function setOfflineError() {
		loadError.value = new Error('Core is currently offline. Start Core to manage this server.')
	}

	async function ensureCoreConnected(): Promise<boolean> {
		const current = core.monitor?.currentStatus
		if (current?.state === 'connected') return true
		try {
			const next = await core.connect()
			return next.state === 'connected'
		} catch {
			return false
		}
	}

	const cachedInstance = queryClient.getQueryData<CoreInstance>(['core-server', instanceId.value])
	if (cachedInstance) {
		rawInstance.value = cachedInstance
		powerState.value = toHostingPowerState(cachedInstance.status)
	}

	async function refreshServer() {
		if (!(await ensureCoreConnected())) {
			setOfflineError()
			return
		}

		try {
			rawInstance.value = await backend.getServer(instanceId.value)
			powerState.value = toHostingPowerState(rawInstance.value.status)
			queryClient.setQueryData(['core-server', instanceId.value], rawInstance.value)
			loadError.value = null
		} catch (error) {
			const isOffline =
				error instanceof CoreOfflineError ||
				(error instanceof Error && /offline|network|fetch|connection/i.test(error.message))
			if (error instanceof CoreApiError && error.status === 404) {
				loadError.value = new Error('This server no longer exists in Core.')
			} else if (isOffline) {
				setOfflineError()
			} else {
				loadError.value = error as Error
				handleError(error as Error)
			}
		}
	}

	async function refreshStats() {
		if (!(await ensureCoreConnected())) return

		try {
			statsData.value = await core.getStats(instanceId.value)
		} catch {
			return
		}
		const next = statsData.value
		if (!next) return

		const cpu = next.cpu_percent ?? 0
		const ramPct = Math.round(((next.memory_mb ?? 0) / Math.max(next.ram_total_mb ?? 1, 1)) * 100)
		uptimeSeconds.value = next.uptime_seconds ?? 0
		cpuHistory.value = [...cpuHistory.value.slice(-9), cpu]
		ramHistory.value = [...ramHistory.value.slice(-9), ramPct]
		stats.value = toStats(next, cpuHistory.value, ramHistory.value, stats.value.current)
	}

	async function connectConsole() {
		if (!(await ensureCoreConnected())) return

		disconnectConsole()
		try {
			const ticket = await core.issueWsTicket()
			socket = await core.openConsole(instanceId.value, ticket)
			consoleState.clear()
			consoleState.beginInitialLogHydration()
			appendSocketListeners(socket, socketListeners, {
				onLog: (line) => {
					const next = [...logLines.value, toLogLine(line)]
					logLines.value =
						next.length > MAX_LOG_LINES ? next.slice(next.length - MAX_LOG_LINES) : next
				},
				onStats: (nextStats) => {
					statsData.value = nextStats
					const cpu = nextStats.cpu_percent ?? 0
					uptimeSeconds.value = nextStats.uptime_seconds ?? 0
					const ramPct = Math.round(
						((nextStats.memory_mb ?? 0) / Math.max(nextStats.ram_total_mb ?? 1, 1)) * 100,
					)
					cpuHistory.value = [...cpuHistory.value.slice(-9), cpu]
					ramHistory.value = [...ramHistory.value.slice(-9), ramPct]
					stats.value = toStats(nextStats, cpuHistory.value, ramHistory.value, stats.value.current)
				},
				onState: (status) => {
					powerState.value = toHostingPowerState(status)
				},
				onOpen: () => {
					isConnected.value = true
				},
				onClose: () => {
					isConnected.value = false
				},
				onError: () => {
					isConnected.value = false
				},
			})
		} catch (error) {
			isConnected.value = false
			console.error('[hosting/core] Failed to connect Core console:', error)
		}
	}

	function disconnectConsole() {
		for (const unlisten of socketListeners) unlisten()
		socketListeners = []
		socket?.close()
		socket = null
		isConnected.value = false
	}

	async function sendCommand(command: string) {
		if (!command.trim()) return
		try {
			if (socket?.readyState === WebSocket.OPEN) socket.send(command)
			else await core.sendCommand(instanceId.value, command)
		} catch (error) {
			handleError(error as Error)
		}
	}

	async function runPowerAction(
		nextState: Archon.Websocket.v0.PowerState,
		action: () => Promise<void>,
	) {
		powerState.value = nextState
		await action().catch(handleError)
		await refreshServer()
	}

	function startServer() {
		return runPowerAction('starting', () => core.start(instanceId.value))
	}

	function stopServer() {
		return runPowerAction('stopping', () => core.stop(instanceId.value))
	}

	function restartServer() {
		return runPowerAction('starting', () => core.restart(instanceId.value))
	}

	async function killServer() {
		await core.kill(instanceId.value).catch(handleError)
		await refreshServer()
	}

	async function repairServer() {
		try {
			await core.repair(instanceId.value)
			addNotification({
				title: 'Repair started',
				text: 'The server is reinstalling. This may take a few minutes.',
				type: 'success',
			})
			await refreshServer()
		} catch (error) {
			handleError(error as Error)
		}
	}

	async function changeVersion(body: CoreChangeVersionBody) {
		try {
			await core.changeVersion(instanceId.value, body)
			addNotification({
				title: 'Version change started',
				text: 'The server is reinstalling with the new version.',
				type: 'success',
			})
			await refreshServer()
		} catch (error) {
			handleError(error as Error)
		}
	}

	async function refreshFsAuth() {
		try {
			const url = await core.adapter.getCoreUrl()
			const token = await core.adapter.getCurrentJwt()
			fsAuth.value = url
				? {
						url: `${url}/instances/${encodeURIComponent(instanceId.value)}/fs`,
						token: token ?? '',
					}
				: null
		} catch {
			fsAuth.value = null
		}
	}

	async function copyId() {
		await navigator.clipboard.writeText(instanceId.value)
		addNotification({
			title: 'Server ID copied',
			text: 'The server ID is in your clipboard.',
			type: 'success',
		})
	}

	provideModrinthServerContext({
		get serverId() {
			return instanceId.value
		},
		worldId,
		server: serverContextServer,
		serverFull: computed(() => null),
		currentUserPermissions: computed(() => 0xffff),
		isConnected,
		isWsAuthIncorrect,
		powerState,
		powerStateDetails,
		isServerRunning,
		stats,
		uptimeSeconds,
		isSyncingContent,
		busyReasons,
		fsAuth,
		fsOps,
		fsQueuedOps,
		refreshFsAuth,
		uploadState,
		cancelUpload,
		activeOperations,
		dismissOperation: async () => {},
	})

	provide(coreServerContextKey, {
		instanceId,
		rawInstance,
		server,
		statsData,
		stats,
		powerState,
		logLines,
		refreshServer,
		refreshStats,
		sendCommand,
		startServer,
		stopServer,
		restartServer,
		killServer,
		repairServer,
		changeVersion,
		copyId,
	})

	async function loadOnlineServerData() {
		await refreshServer()
		await refreshStats()
		await connectConsole()
		await refreshFsAuth()
	}

	onMounted(async () => {
		unsubscribeCoreStatus = core.monitor?.onStatus((next) => {
			if (next.state !== 'connected') {
				disconnectConsole()
				setOfflineError()
				return
			}
			void loadOnlineServerData()
		})

		if (await ensureCoreConnected()) await loadOnlineServerData()
		else setOfflineError()
	})

	watch(powerState, (next, prev) => {
		if (next === prev) return
		if ((next === 'starting' || next === 'running') && !isConnected.value) {
			void connectConsole()
		}
	})

	watch(
		[server, instanceId],
		() => {
			serverContextServer.value =
				server.value ?? toHostingServer(createFallbackInstance(instanceId.value))
		},
		{ immediate: true },
	)

	watch(instanceId, () => {
		disconnectConsole()
		rawInstance.value = null
		statsData.value = null
		uptimeSeconds.value = 0
		loadError.value = null
		void loadOnlineServerData()
	})

	onUnmounted(() => {
		unsubscribeCoreStatus?.()
		disconnectConsole()
	})

	return {
		instanceId,
		server,
		loadError,
		statsData,
		powerState,
		startServer,
		stopServer,
		restartServer,
		killServer,
		repairServer,
		refreshServer,
		changeVersion,
		copyId,
	}
}
