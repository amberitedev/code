<template>
	<div class="relative flex select-none flex-col gap-6" data-pyro-server-manager-root>
		<div class="flex flex-col gap-4">
			<ServerManageStats :data="ctx.stats.value" :loading="!isConnected" />

			<div class="flex min-h-[700px] flex-col gap-2">
				<span class="text-2xl font-semibold text-contrast">Console</span>
				<ConsolePageLayout />
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { ConsolePageLayout, provideConsoleManager, ServerManageStats } from '@modrinth/ui'
import { computed, inject, ref } from 'vue'

import { coreServerContextKey } from './server/core-server-instance'

const ctx = inject(coreServerContextKey)
if (!ctx) throw new Error('Missing Core server context')

const isConnected = computed(
	() => ctx.powerState.value !== 'stopped' || ctx.logLines.value.length > 0,
)

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
