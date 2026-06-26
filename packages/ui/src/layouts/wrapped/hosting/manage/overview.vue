<template>
	<div class="relative flex select-none flex-col gap-6" data-core-server-manager-root>
		<div class="flex flex-col gap-4">
			<ServerManageStats
				:data="ctx.stats.value"
				:loading="!ctx.statsData.value"
				:storage-link="`${basePath}/files`"
			/>

			<ServerQueryCard
				:instance-id="ctx.instanceId.value"
				:enabled="ctx.powerState.value === 'running'"
			/>

			<div class="flex min-h-[700px] flex-col gap-2">
				<span class="text-2xl font-semibold text-contrast">Console</span>
				<ConsolePageLayout />
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'

import ServerManageStats from '#ui/components/servers/ServerManageStats.vue'
import { useModrinthServersConsole } from '#ui/composables'
import ConsolePageLayout from '#ui/layouts/shared/console/layout.vue'
import { provideConsoleManager } from '#ui/layouts/shared/console/providers'

import { injectCoreServerContext } from './context'
import ServerQueryCard from './ServerQueryCard.vue'

defineProps<{
	showAdvancedDebugInfo?: boolean
}>()

const route = useRoute()
const ctx = injectCoreServerContext()
const consoleState = useModrinthServersConsole()
const basePath = computed(() => route.path.replace(/\/(content|files|backups|access)\/?$/, ''))

provideConsoleManager({
	logLines: consoleState.output,
	sendCommand: (command) => {
		void ctx.sendCommand(command)
	},
	showCommandInput: true,
	disableCommandInput: computed(() => ctx.powerState.value !== 'running'),
	loading: computed(() => false),
	onClear: () => {
		consoleState.clear()
	},
	shareDisabled: computed(() => false),
	emptyStateType: 'server',
	crashAnalysis: ref(null),
})
</script>
