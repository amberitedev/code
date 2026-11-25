<template>
	<div
		class="relative flex select-none flex-col gap-6"
		:class="fillConsole ? 'flex-1' : ''"
		data-pyro-server-manager-root
	>
		<div class="flex flex-col gap-4" :class="fillConsole ? 'flex-1' : ''">
			<div class="flex flex-col gap-2" :class="fillConsole ? 'flex-1' : 'min-h-[700px]'">
				<span class="shrink-0 text-2xl font-semibold text-contrast">Console</span>

				<ConsolePageLayout :min-log-height="fillConsole ? '28rem' : undefined" />
			</div>
		</div>

		<button
			v-if="showAdvancedDebugInfo"
			class="self-start rounded-lg bg-surface-3 px-3 py-1 text-sm text-contrast hover:brightness-125"
			@click="downloadLog4jDebug"
		>
			Download WS debug JSON
		</button>
	</div>
</template>

<script setup lang="ts">
import { core as coreRoutes, instanceFiles, type Mclogs } from '@modrinth/api-client'
import { useStorage } from '@vueuse/core'
import { computed, ref, watch } from 'vue'

import { useModrinthServersConsole } from '#ui/composables'
import { useServerPermissions } from '#ui/composables/server-permissions'
import {
	ConsolePageLayout,
	provideConsoleManager,
	useInstanceLogs,
} from '#ui/layouts/shared/console'
import {
	injectModrinthClient,
	injectModrinthServerContext,
	injectNotificationManager,
} from '#ui/providers'

const props = withDefaults(
	defineProps<{
		showAdvancedDebugInfo?: boolean
		fillConsole?: boolean
	}>(),
	{
		showAdvancedDebugInfo: false,
		fillConsole: false,
	},
)

const client = injectModrinthClient()
const {
	serverId,
	core,
	isConnected,
	isServerRunning,
	powerState: serverPowerState,
} = injectModrinthServerContext()
const modrinthServersConsole = useModrinthServersConsole()
const { handleError } = injectNotificationManager()
const logs = useInstanceLogs({
	client: core,
	instanceId: serverId,
	live: modrinthServersConsole,
	running: isServerRunning,
	queryKey: ['servers', 'logs', serverId],
	onError: handleError,
})
// Running commands and clearing the console need more than the right to start the server.
const { canExecuteCommands, permissionDeniedMessage } = useServerPermissions()

watch(
	() => props.showAdvancedDebugInfo,
	(enabled) => {
		modrinthServersConsole.setWsEventCaptureEnabled(enabled)
	},
	{ immediate: true },
)

const crashAnalysis = ref<Mclogs.Insights.v1.InsightsResponse | null>(null)
const DISMISS_DURATION_MS = 30 * 60 * 1000
const dismissedUntil = useStorage(`modrinth-crash-dismissed-${serverId}`, 0)

const isDismissed = () => Date.now() < dismissedUntil.value

const inspectError = async () => {
	if (isDismissed()) return

	try {
		const blob = await core.call(instanceFiles.ReadInstanceFile, {
			params: { id: serverId },
			query: { path: 'logs/latest.log' },
		})
		const log = await blob.text()
		if (!log) return

		const data = await client.mclogs.insights_v1.analyse(log)
		if (data.analysis?.problems?.length) {
			crashAnalysis.value = data
		} else {
			crashAnalysis.value = null
		}
	} catch (error) {
		console.error('Failed to analyze logs:', error)
		crashAnalysis.value = null
	}
}

const dismissCrash = () => {
	dismissedUntil.value = Date.now() + DISMISS_DURATION_MS
	crashAnalysis.value = null
}

provideConsoleManager({
	logLines: logs.logLines,
	logSources: logs.logSources,
	activeLogSourceIndex: logs.activeLogSourceIndex,
	onDelete: logs.onDelete,
	deleteDisabled: logs.deleteDisabled,
	sendCommand: (command: string) => {
		if (!canExecuteCommands.value) return
		core
			.call(coreRoutes.SendCommand, { params: { id: serverId }, body: { command } })
			.catch(handleError)
	},
	showCommandInput: logs.isLive,
	disableCommandInput: computed(
		() => !canExecuteCommands.value || serverPowerState.value !== 'running',
	),
	disableCommandInputTooltip: computed(() =>
		canExecuteCommands.value ? undefined : permissionDeniedMessage.value,
	),
	loading: computed(() => !isConnected.value || modrinthServersConsole.isInitialLogHydrating.value),
	// A Core keeps no buffer of its own: the console reads the server's log, which stays.
	onClear: () => {
		if (canExecuteCommands.value && logs.isLive.value) modrinthServersConsole.clear()
	},
	clearDisabled: computed(() => !canExecuteCommands.value),
	clearDisabledTooltip: computed(() =>
		canExecuteCommands.value ? undefined : permissionDeniedMessage.value,
	),
	shareDisabled: computed(() => !isConnected.value),
	emptyStateType: 'server',
	crashAnalysis,
	onDismissCrash: dismissCrash,
})

watch(
	() => serverPowerState.value,
	(newVal) => {
		// A run that ended wrote a log of its own.
		if (newVal === 'stopped' || newVal === 'crashed') void logs.refresh()
		if (newVal === 'crashed') {
			void inspectError()
		} else {
			crashAnalysis.value = null
		}
	},
)

if (serverPowerState.value === 'crashed') {
	void inspectError()
}

const downloadLog4jDebug = () => {
	const events = modrinthServersConsole.getWsEventHistory()
	const blob = new Blob([JSON.stringify(events, null, 2)], { type: 'application/json' })
	const url = URL.createObjectURL(blob)
	const a = document.createElement('a')
	a.href = url
	a.download = `ws-debug-${serverId}-${Date.now()}.json`
	document.body.appendChild(a)
	a.click()
	document.body.removeChild(a)
	URL.revokeObjectURL(url)
}
</script>
