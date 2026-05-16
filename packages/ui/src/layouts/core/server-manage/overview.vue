<template>
	<div class="flex flex-col gap-6">
		<section class="grid gap-4 md:grid-cols-3">
			<div class="rounded-2xl bg-surface-3 p-5">
				<p class="text-sm font-semibold text-secondary">CPU</p>
				<p class="text-3xl font-bold text-contrast">{{ cpuPercent }}</p>
			</div>
			<div class="rounded-2xl bg-surface-3 p-5">
				<p class="text-sm font-semibold text-secondary">Memory</p>
				<p class="text-3xl font-bold text-contrast">{{ memoryUsage }}</p>
			</div>
			<div class="rounded-2xl bg-surface-3 p-5">
				<p class="text-sm font-semibold text-secondary">Connection</p>
				<p class="text-3xl font-bold text-contrast">{{ isConnected ? 'Online' : 'Offline' }}</p>
			</div>
		</section>

		<section class="flex min-h-[620px] flex-col gap-3">
			<h2 class="text-2xl font-semibold text-contrast">Console</h2>
			<ConsolePageLayout />
		</section>
	</div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import { useFormatBytes } from '#ui/composables/format-bytes'
import { useModrinthServersConsole } from '#ui/composables/server-console'
import { ConsolePageLayout, provideConsoleManager } from '#ui/layouts/shared/console'
import { injectCoreClient, injectModrinthServerContext } from '#ui/providers'

const coreClient = injectCoreClient()
const consoleState = useModrinthServersConsole()
const formatBytes = useFormatBytes()
const { serverId, isConnected, stats, powerState } = injectModrinthServerContext()
const crashAnalysis = ref(null)

const cpuPercent = computed(() => `${(stats.value.current.cpu_percent ?? 0).toFixed(2)}%`)
const memoryUsage = computed(() => {
	const current = stats.value.current
	return `${formatBytes(current.ram_usage_bytes ?? 0, 1)} / ${formatBytes(current.ram_total_bytes ?? 0, 1)}`
})

provideConsoleManager({
	logLines: consoleState.output,
	sendCommand: (command) => {
		coreClient.sendCommand(serverId, command).catch((error) => {
			console.error('[core/server-manage] Failed to send command:', error)
		})
	},
	showCommandInput: true,
	disableCommandInput: computed(() => powerState.value !== 'running'),
	loading: computed(() => !isConnected.value),
	onClear: async () => consoleState.clear(),
	shareDisabled: computed(() => !isConnected.value),
	emptyStateType: 'server',
	crashAnalysis,
	onDismissCrash: () => {},
})

watch(powerState, (nextState) => {
	if (nextState !== 'crashed') crashAnalysis.value = null
})
</script>
