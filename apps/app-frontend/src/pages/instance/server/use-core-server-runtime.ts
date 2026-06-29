import type {
	ConnectionStatus,
	CoreChangeVersionBody,
	CoreInstance,
	CoreInstanceStatus,
	CoreStats,
	CoreWsConnection,
} from '@amberite/amberite-api'
import { CoreApiError, CoreOfflineError } from '@amberite/amberite-api'
import type { Archon } from '@modrinth/api-client'
import { injectNotificationManager } from '@modrinth/ui'
import type { Stats } from '@modrinth/utils'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, isRef, onMounted, onUnmounted, type Ref, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import { useCoreClient } from '@/composables/useCoreClient'
import {
	patchCoreInstanceInCache,
	upsertCoreInstanceInCache,
} from '@/composables/useCoreInstances'

import {
	appendSocketListeners,
	toHostingPowerState,
	toHostingServer,
	toLogLine,
	toStats,
} from './core-server-instance'
import { provideCoreServerRuntime } from './core-server-providers'
import type { ServerSettingsTabId } from './settings/tabs'

export function useCoreServerRuntime(
	instanceIdOverride?: string | Ref<string | null | undefined>,
	publicPathOverride?: string | Ref<string | null | undefined>,
) {
	const route = useRoute()
	const core = useCoreClient()
	const queryClient = useQueryClient()
	const { addNotification, handleError } = injectNotificationManager()

	const instanceId = computed(() => {
		if (isRef(instanceIdOverride)) return instanceIdOverride.value ?? (route.params.id as string)
		return instanceIdOverride ?? (route.params.id as string)
	})
	const publicPath = computed(() => {
		if (isRef(publicPathOverride)) return publicPathOverride.value ?? null
		return publicPathOverride ?? null
	})
	const rawInstance = ref<CoreInstance | null>(null)
	const server = computed(() => (rawInstance.value ? toHostingServer(rawInstance.value) : null))
	const publicInstanceId = computed(
		() =>
			rawInstance.value?.path ??
			publicPath.value ??
			(isUuid(instanceId.value) ? '' : instanceId.value),
	)
	const loadError = ref<Error | null>(null)
	const statsData = ref<CoreStats | null>(null)
	const stats = ref<Stats>(toStats(null))
	const powerState = ref<Archon.Websocket.v0.PowerState>('stopped')
	const coreStatus = ref<ConnectionStatus | null>(core.monitor?.currentStatus ?? null)
	const isConnected = ref(false)
	const isWsAuthIncorrect = ref(false)
	const isSyncingContent = ref(false)
	const logLines = ref<ReturnType<typeof toLogLine>[]>([])
	const MAX_LOG_LINES = 5000
	const cpuHistory = ref<number[]>([])
	const ramHistory = ref<number[]>([])
	let socket: CoreWsConnection | null = null
	let socketListeners: Array<() => void> = []
	let unsubscribeCoreStatus: (() => void) | undefined

	const isCoreConnected = computed(() => coreStatus.value?.state === 'connected')

	function setOfflineError() {
		loadError.value = new Error('Core is currently offline. Start Core to manage this server.')
	}

	const cachedInstance = queryClient.getQueryData<CoreInstance>(['core-server', instanceId.value])
	if (cachedInstance) {
		rawInstance.value = cachedInstance
		powerState.value = toHostingPowerState(cachedInstance.status)
	}

	async function refreshServer() {
		if (!isCoreConnected.value) {
			setOfflineError()
			return
		}

		try {
			rawInstance.value = await core.getInstance(instanceId.value)
			powerState.value = toHostingPowerState(rawInstance.value.status)
			queryClient.setQueryData(['core-server', instanceId.value], rawInstance.value)
			if (publicInstanceId.value && publicInstanceId.value !== instanceId.value) {
				queryClient.setQueryData(['core-server', publicInstanceId.value], rawInstance.value)
			}
			upsertCoreInstanceInCache(queryClient, rawInstance.value)
			loadError.value = null
		} catch (error) {
			const isOffline =
				error instanceof CoreOfflineError ||
				(error instanceof Error && /offline|network|fetch|connection/i.test(error.message))
			if (error instanceof CoreApiError && error.status === 404) {
				loadError.value = new Error('This server no longer exists in Core.')
			} else if (isOffline) {
				coreStatus.value = core.monitor?.currentStatus ?? coreStatus.value
				setOfflineError()
			} else {
				loadError.value = error as Error
				handleError(error as Error)
			}
		}
	}

	async function refreshStats() {
		if (!isCoreConnected.value) return

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
		if (!isCoreConnected.value) return

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
		if (rawInstance.value) {
			patchCoreInstanceInCache(queryClient, rawInstance.value.id, { status: 'starting' })
		}
		await core.start(instanceId.value).catch(handleError)
		await refreshServer()
	}

	async function stopServer() {
		powerState.value = 'stopping'
		if (rawInstance.value) {
			patchCoreInstanceInCache(queryClient, rawInstance.value.id, { status: 'stopping' })
		}
		await core.stop(instanceId.value).catch(handleError)
		await refreshServer()
	}

	async function restartServer() {
		powerState.value = 'starting'
		if (rawInstance.value) {
			patchCoreInstanceInCache(queryClient, rawInstance.value.id, { status: 'starting' })
		}
		await core.restart(instanceId.value).catch(handleError)
		await refreshServer()
	}

	async function killServer() {
		if (rawInstance.value) {
			patchCoreInstanceInCache(queryClient, rawInstance.value.id, { status: 'offline' })
		}
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
		if (!isCoreConnected.value) {
			fsAuth.value = null
			return
		}

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
		if (!rawInstance.value && isCoreConnected.value) {
			await refreshServer()
		}
		if (!publicInstanceId.value) {
			handleError(new Error('Server path is unavailable while Core is offline.'))
			return
		}
		await navigator.clipboard.writeText(publicInstanceId.value)
		addNotification({
			title: 'Server path copied',
			text: 'The server path is in your clipboard.',
			type: 'success',
		})
	}

	function isUuid(value: string): boolean {
		return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
			value,
		)
	}

	const { settingsController, fsAuth } = provideCoreServerRuntime({
		instanceId,
		publicInstanceId,
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

	async function loadOnlineServerData() {
		await refreshServer()
		await refreshStats()
		if (isCoreConnected.value) {
			void queryClient.prefetchQuery({
				queryKey: ['core-mods', instanceId.value],
				queryFn: () => core.listMods(instanceId.value),
				staleTime: 30_000,
			})
			await connectConsole()
		}
		await refreshFsAuth()
	}

	onMounted(async () => {
		unsubscribeCoreStatus = core.monitor?.onStatus((next) => {
			const wasConnected = isCoreConnected.value
			coreStatus.value = next
			if (next.state !== 'connected') {
				disconnectConsole()
				setOfflineError()
				return
			}
			if (!wasConnected) void loadOnlineServerData()
		})

		const currentStatus = core.monitor?.currentStatus ?? (await core.connect().catch(() => null))
		coreStatus.value = currentStatus
		if (currentStatus?.state === 'connected') {
			await loadOnlineServerData()
		} else {
			setOfflineError()
		}
	})

	watch(powerState, (next, prev) => {
		if (next === prev) return
		if ((next === 'starting' || next === 'running') && !isConnected.value && isCoreConnected.value) {
			void connectConsole()
		}
	})

	watch(instanceId, () => {
		disconnectConsole()
		rawInstance.value = null
		statsData.value = null
		loadError.value = null
		if (isCoreConnected.value) void loadOnlineServerData()
	})

	onUnmounted(() => {
		unsubscribeCoreStatus?.()
		disconnectConsole()
	})

	return {
		instanceId,
		publicInstanceId,
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
		openSettings: (tabId?: ServerSettingsTabId) => settingsController.open(tabId),
	}
}
