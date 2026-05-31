<template>
	<div class="relative flex select-none flex-col gap-6" data-pyro-server-manager-root>
		<div class="flex flex-col gap-4">
			<ServerStats
				:data="ctx.stats.value"
				:loading="!ctx.statsData.value"
				:storage-link="filesLink"
			/>

			<div class="flex min-h-[700px] flex-col gap-2">
				<span class="text-2xl font-semibold text-contrast">Console</span>
				<ConsolePageLayout />
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { ConsolePageLayout, provideConsoleManager } from '@modrinth/ui'
import { computed, inject, ref } from 'vue'

import { coreServerContextKey } from './server/core-server-instance'
import ServerStats from './server/ServerStats.vue'

const ctx = inject(coreServerContextKey)
if (!ctx) throw new Error('Missing Core server context')

const filesLink = computed(() => `/instance/${encodeURIComponent(ctx.instanceId.value)}/files`)

provideConsoleManager({
	logLines: ctx.logLines,
	sendCommand: (command) => {
		void ctx.sendCommand(command)
	},
	showCommandInput: true,
	disableCommandInput: computed(() => ctx.powerState.value !== 'running'),
	loading: computed(() => false),
	onClear: () => {
		ctx.logLines.value = []
	},
	shareDisabled: computed(() => false),
	emptyStateType: 'server',
	crashAnalysis: ref(null),
})
</script>
