<template>
	<div class="flex flex-col gap-4 h-full">
		<ConsolePageLayout />
	</div>
</template>

<script setup lang="ts">
import type { Mclogs } from '@modrinth/api-client'
import {
	ConsolePageLayout,
	injectModrinthClient,
	injectNotificationManager,
	provideConsoleManager,
	useInstanceLogs,
} from '@modrinth/ui'
import { ref } from 'vue'

import { useAppEvent } from '@/composables/use-app-event'
import { useInstanceConsole } from '@/composables/useInstanceConsole'
import { apiClient } from '@/platform/modrinth-client'

import { injectInstancePage } from '../instance-context'
import { instanceKeys } from '../query-options'

const client = injectModrinthClient()
const { handleError } = injectNotificationManager()
const instancePage = injectInstancePage()
const instanceId = instancePage.instanceId
const { liveConsole, hydrate, clearLive } = useInstanceConsole(instanceId.value)

// Not awaited, so the tab opens at once: the live console is a per-instance store, and the
// terminal fills in when the buffered output arrives. Live `log` events are ignored until then
// because the buffer already contains them.
let liveConsoleHydrated = false
const liveConsoleHydration = hydrate()
	.catch(handleError)
	.finally(() => {
		liveConsoleHydrated = true
	})

const logs = useInstanceLogs({
	client: apiClient.local,
	instanceId,
	live: liveConsole,
	running: instancePage.playing,
	queryKey: () => instanceKeys.logs(instanceId.value),
	onError: handleError,
})

const crashAnalysis = ref<Mclogs.Insights.v1.InsightsResponse | null>(null)

async function analyseForCrash() {
	const lines = liveConsole.output.value
	if (lines.length === 0) return

	const content = lines.map((l) => l.text).join('\n')
	try {
		const data = await client.mclogs.insights_v1.analyse(content)
		if (data.analysis?.problems?.length > 0) {
			crashAnalysis.value = data
		}
	} catch {
		// Crash analysis is best-effort
	}
}

provideConsoleManager({
	logLines: logs.logLines,
	logSources: logs.logSources,
	activeLogSourceIndex: logs.activeLogSourceIndex,
	showCommandInput: false,
	loading: ref(false),
	onClear: () => {
		if (!logs.isLive.value) return
		void clearLive()
	},
	onDelete: logs.onDelete,
	deleteDisabled: logs.deleteDisabled,
	deleteDisabledTooltip: 'Cannot delete latest.log while the instance is running',
	shareDisabled: instancePage.offline,
	emptyStateType: 'instance',
	crashAnalysis,
	onDismissCrash: () => {
		crashAnalysis.value = null
	},
})

void liveConsoleHydration.then(() => {
	if (!instancePage.playing.value) void analyseForCrash()
})

useAppEvent('log', (payload) => {
	if (payload.instance_id !== instanceId.value || !liveConsoleHydrated) return

	if (payload.type === 'log4j') {
		liveConsole.addLog4jEvent(payload)
	} else if (payload.type === 'legacy') {
		liveConsole.addLegacyLog(payload.message)
	}
})

// The buffer holds this run's whole live log, so it replaces lines a resync lost.
useAppEvent('resync', async () => {
	await liveConsoleHydration
	void logs.refresh()
	liveConsoleHydrated = false
	liveConsole.clear()
	await hydrate().catch(handleError)
	liveConsoleHydrated = true
})

useAppEvent('process', async (e) => {
	if (e.instance_id !== instanceId.value) return
	if (e.event === 'launched') {
		liveConsole.clear()
		logs.activeLogSourceIndex.value = 0
		void logs.refresh()
	}
	if (e.event === 'finished') {
		await logs.refresh()
		void analyseForCrash()
	}
})
</script>
