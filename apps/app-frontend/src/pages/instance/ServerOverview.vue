<template>
	<div class="relative flex select-none flex-col gap-6" data-core-server-manager-root>
		<div class="flex flex-col gap-4">
			<ServerManageStats
				:data="ctx.stats.value"
				:loading="!ctx.statsData.value"
				:storage-link="`${basePath}/files`"
			/>

			<div class="flex min-h-[700px] flex-col gap-2">
				<span class="text-2xl font-semibold text-contrast">Console</span>
				<div
					v-if="showServerNotRunningState"
					class="flex min-h-0 flex-1 items-center justify-center rounded-[20px] border border-solid border-surface-4 bg-surface-2"
				>
					<EmptyState
						type="no-connection"
						heading="Server not up"
						description="Start the server to see live console output."
					/>
				</div>
				<ConsolePageLayout v-else />
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {
	ConsolePageLayout,
	EmptyState,
	injectSharedCoreServerContext,
} from '@modrinth/ui'
import ServerManageStats from '@modrinth/ui/src/components/servers/ServerManageStats.vue'
import { provideConsoleManager } from '@modrinth/ui/src/layouts/shared/console/providers'
import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'

const route = useRoute()
const ctx = injectSharedCoreServerContext()
const basePath = computed(() => route.path.replace(/\/(content|files|backups|access|browse)\/?$/, ''))
const showServerNotRunningState = computed(
	() => !['running', 'starting'].includes(ctx.powerState.value),
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
	crashAnalysis: ref(null),
})
</script>
