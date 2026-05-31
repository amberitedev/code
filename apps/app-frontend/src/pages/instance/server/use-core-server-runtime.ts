import type {
	CoreInstance,
	CoreInstanceStatus,
	CoreStats,
	CoreWsConnection,
} from '@amberite/amberite-api'
import type { Archon } from '@modrinth/api-client'
import { injectNotificationManager } from '@modrinth/ui'
import type { Stats } from '@modrinth/utils'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, onMounted, onUnmounted, ref } from 'vue'
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

export function useCoreServerRuntime() {
	const route = useRoute()
	const core = useCoreClient()
	const queryClient = useQueryClient()
	const { addNotification, handleError } = injectNotificationManager()

	const instanceId = computed(() => route.params.id as string)
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
	let socket: CoreWsConnection | null = null
	let socketListeners: Array<() => void> = []
	let statsTimer: ReturnType<typeof setInterval> | null = null

	async function refreshServer() {
		try {
			loadError.value = null
			rawInstance.value = await core.getInstance(instanceId.value)
			powerState.value = toHostingPowerState(rawInstance.value.status)
			queryClient.setQueryData(['core-server', instanceId.value], rawInstance.value)
		} catch (error) {
			loadError.value = error as Error
			handleError(error as Error)
		}
	}

	async function refreshStats() {
		try {
			statsData.value = await core.getStats(instanceId.value)
			stats.value = toStats(statsData.value)
		} catch {
			statsData.value = null
		}
	}

	async function connectConsole() {
		disconnectConsole()
		try {
			const ticket = await core.issueWsTicket()
			socket = await core.openConsole(instanceId.value, ticket)
			appendSocketListeners(socket, socketListeners, {
				onLog: (line) => logLines.value.push(toLogLine(line)),
				onStats: (nextStats) => {
					statsData.value = nextStats
					stats.value = toStats(nextStats)
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

	async function copyId() {
		await navigator.clipboard.writeText(instanceId.value)
		addNotification({
			title: 'Server ID copied',
			text: 'The server ID is in your clipboard.',
			type: 'success',
		})
	}

	provideCoreServerRuntime({
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
	})

	onMounted(async () => {
		await refreshServer()
		await refreshStats()
		await connectConsole()
		statsTimer = setInterval(refreshStats, 5000)
	})

	onUnmounted(() => {
		disconnectConsole()
		if (statsTimer) clearInterval(statsTimer)
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
		copyId,
	}
}
