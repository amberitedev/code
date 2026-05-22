<template>
	<div class="relative flex select-none flex-col gap-6" data-pyro-server-manager-root>
		<div class="flex flex-col gap-4">
			<ServerManageStats :data="stats" :loading="false" />

			<div class="flex min-h-[700px] flex-col gap-2">
				<span class="text-2xl font-semibold text-contrast">Console</span>
				<ConsolePageLayout />
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import { useModrinthServersConsole } from '#ui/composables/server-console'
import { ConsolePageLayout, provideConsoleManager } from '#ui/layouts/shared/console'
import { injectCoreClient, injectModrinthServerContext } from '#ui/providers'

import ServerManageStats from '../../wrapped/hosting/manage/components/ServerManageStats.vue'

const coreClient = injectCoreClient()
const consoleState = useModrinthServersConsole()
const { serverId, isConnected, stats, powerState } = injectModrinthServerContext()
const crashAnalysis = ref(null)

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
