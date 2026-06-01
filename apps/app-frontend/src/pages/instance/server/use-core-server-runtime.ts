import type {
	CoreChangeVersionBody,
	CoreInstance,
	CoreInstanceStatus,
	CoreStats,
	CoreWsConnection,
} from '@amberite/amberite-api'
import { CoreOfflineError } from '@amberite/amberite-api'
import type { Archon } from '@modrinth/api-client'
import { injectNotificationManager } from '@modrinth/ui'
import type { Stats } from '@modrinth/utils'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import { useCoreClient } from '@/composables/useCoreClient'

import {
	appendSocketListeners,
	toHostingPowerState,
	toHostingServer,
	toLogLine,
	toStats,
} from './core-server-instance'
import { provideCoreServerRuntime } from './core-server-providers'
import type { ServerSettingsTabId } from './settings/tabs'

export function useCoreServerRuntime(instanceIdOverride?: string) {
	const route = useRoute()
	const core = useCoreClient()
	const queryClient = useQueryClient()
	const { addNotification, handleError } = injectNotificationManager()

	const instanceId = computed(() => instanceIdOverride ?? (route.params.id as string))
	const rawInstance = ref<CoreInstance | null>(null)
	const server = computed(() => (rawInstance.value ? toHostingServer(rawInstance.value) : null))
	const loadError = ref<Error | null>(null)
	const statsData = ref<CoreStats | null>(null)
	const stats = ref<Stats>(toStats(null))
	const powerState = ref<Archon.Websocket.v0.PowerState>('stopped')
	const isConnected = ref(false)
	const isWsAuthIncorrect = ref(false)
	const isSyncingContent = ref(false)
	const logLines = ref<ReturnType<typeof toLogLine>[]>([])
	const MAX_LOG_LINES = 5000
	const cpuHistory = ref<number[]>([])
	const ramHistory = ref<number[]>([])
	let socket: CoreWsConnection | null = null
	let socketListeners: Array<() => void> = []
	let statsTimer: ReturnType<typeof setInterval> | null = null
	let serverTimer: ReturnType<typeof setInterval> | null = null

	const cachedInstance = queryClient.getQueryData<CoreInstance>(['core-server', instanceId.value])
	if (cachedInstance) {
		rawInstance.value = cachedInstance
		powerState.value = toHostingPowerState(cachedInstance.status)
	}

	async function refreshServer() {
		try {
			loadError.value = null
			rawInstance.value = await core.getInstance(instanceId.value)
			powerState.value = toHostingPowerState(rawInstance.value.status)
			queryClient.setQueryData(['core-server', instanceId.value], rawInstance.value)
		} catch (error) {
			const isOffline =
				error instanceof CoreOfflineError ||
				(error instanceof Error && /offline|network|fetch|connection/i.test(error.message))
			if (isOffline) {
				loadError.value = new Error('Core is currently offline. Start Core to manage this server.')
			} else {
				loadError.value = error as Error
				handleError(error as Error)
			}
		}
	}

	async function refreshStats() {
		try {
			statsData.value = await core.getStats(instanceId.value)
		} catch {
			// Keep previous statsData so loading state doesn't flash; only
			// overwrite on success.
		}
		const next = statsData.value
		if (next) {
			const cpu = next.cpu_percent ?? 0
			const ramPct = Math.round(((next.memory_mb ?? 0) / Math.max(next.ram_total_mb ?? 1, 1)) * 100)
			cpuHistory.value = [...cpuHistory.value.slice(-9), cpu]
			ramHistory.value = [...ramHistory.value.slice(-9), ramPct]
			stats.value = toStats(next, cpuHistory.value, ramHistory.value, stats.value.current)
		}
	}

	async function connectConsole() {
		disconnectConsole()
		try {
			const ticket = await core.issueWsTicket()
			socket = await core.openConsole(instanceId.value, ticket)
			appendSocketListeners(socket, socketListeners, {
				onLog: (line) => {
					const next = [...logLines.value, toLogLine(line)]
					logLines.value =
						next.length > MAX_LOG_LINES ? next.slice(next.length - MAX_LOG_LINES) : next
				},
				onStats: (nextStats) => {
					statsData.value = nextStats
					const cpu = nextStats.cpu_percent ?? 0
					const ramPct = Math.round(
						((nextStats.memory_mb ?? 0) / Math.max(nextStats.ram_total_mb ?? 1, 1)) * 100,
					)
					cpuHistory.value = [...cpuHistory.value.slice(-9), cpu]
					ramHistory.value = [...ramHistory.value.slice(-9), ramPct]
					stats.value = toStats(nextStats, cpuHistory.value, ramHistory.value, stats.value.current)
				},
				onState: (status: CoreInstanceStatus) => {
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
			console.error('[server-instance] Failed to connect Core console:', error)
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

	async function startServer() {
		powerState.value = 'starting'
		await core.start(instanceId.value).catch(handleError)
		await refreshServer()
	}

	async function stopServer() {
		powerState.value = 'stopping'
		await core.stop(instanceId.value).catch(handleError)
		await refreshServer()
	}

	async function restartServer() {
		powerState.value = 'starting'
		await core.restart(instanceId.value).catch(handleError)
		await refreshServer()
	}

	async function killServer() {
		await core.kill(instanceId.value).catch(handleError)
		await refreshServer()
	}

	async function repairServer() {
		console.log('[server-instance] Repairing (reinstalling) instance', instanceId.value)
		try {
			await core.repair(instanceId.value)
			addNotification({
				title: 'Repair started',
				text: 'The server is reinstalling. This may take a few minutes.',
				type: 'success',
			})
			await refreshServer()
		} catch (error) {
			console.error('[server-instance] Repair failed:', error)
			handleError(error as Error)
		}
	}

	async function changeVersion(body: CoreChangeVersionBody) {
		console.log('[server-instance] Changing version for', instanceId.value, body)
		try {
			await core.changeVersion(instanceId.value, body)
			addNotification({
				title: 'Version change started',
				text: 'The server is reinstalling with the new version.',
				type: 'success',
			})
			await refreshServer()
		} catch (error) {
			console.error('[server-instance] Change version failed:', error)
			handleError(error as Error)
		}
	}

	async function refreshFsAuth() {
		try {
			const url = await core.adapter.getCoreUrl()
			const token = await core.adapter.getCurrentJwt()
			if (url) {
				fsAuth.value = {
					url: `${url}/instances/${encodeURIComponent(instanceId.value)}/fs`,
					token: token ?? '',
				}
			} else {
				fsAuth.value = null
			}
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

	const { settingsController, fsAuth } = provideCoreServerRuntime({
		instanceId,
		rawInstance,
		server,
		statsData,
		stats,
		powerState,
		isConnected,
		isWsAuthIncorrect,
		isSyncingContent,
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
		refreshFsAuth,
	})

	onMounted(async () => {
		await refreshServer()
		await refreshStats()
		void queryClient.prefetchQuery({
			queryKey: ['core-mods', instanceId.value],
			queryFn: () => core.listMods(instanceId.value),
			staleTime: 30_000,
		})
		await connectConsole()
		statsTimer = setInterval(refreshStats, 5000)
		serverTimer = setInterval(refreshServer, 10_000)
		await refreshFsAuth()
	})

	watch(powerState, (next, prev) => {
		if (next === prev) return
		if ((next === 'starting' || next === 'running') && !isConnected.value) {
			void connectConsole()
		}
	})

	onUnmounted(() => {
		disconnectConsole()
		if (statsTimer) clearInterval(statsTimer)
		if (serverTimer) clearInterval(serverTimer)
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
		changeVersion,
		copyId,
		openSettings: (tabId?: ServerSettingsTabId) => settingsController.open(tabId),
	}
}
