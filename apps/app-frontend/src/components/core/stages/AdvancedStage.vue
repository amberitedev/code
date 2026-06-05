<script setup lang="ts">
import { FolderOpenIcon, HistoryIcon, RocketIcon, ShieldIcon } from '@modrinth/assets'
import { StyledInput, Toggle } from '@modrinth/ui'

import { useOnboarding } from '../core-onboarding-context'

const ctx = useOnboarding()
</script>

<template>
	<div class="flex flex-col gap-4">
		<div class="grid gap-4 md:grid-cols-2">
			<div class="rounded-2xl bg-surface-3 p-5">
				<div class="mb-4 flex items-center gap-3">
					<RocketIcon class="text-brand" />
					<h2 class="m-0 text-xl font-bold text-contrast">Runtime</h2>
				</div>
				<div v-if="ctx.mode === 'setup'" class="flex flex-col gap-4">
					<div class="flex items-center justify-between gap-4">
						<div>
							<div class="font-bold text-contrast">Start on startup</div>
							<p class="m-0 text-sm text-secondary">Launch Core when Windows starts.</p>
						</div>
						<Toggle v-model="ctx.runOnStartup.value" />
					</div>
					<div class="flex items-center justify-between gap-4">
						<div>
							<div class="font-bold text-contrast">Run in background</div>
							<p class="m-0 text-sm text-secondary">Keep Core alive after closing the app.</p>
						</div>
						<Toggle v-model="ctx.runInBackground.value" />
					</div>
				</div>
				<label v-else class="flex flex-col gap-2">
					<span class="text-sm font-bold text-contrast">Connection policy</span>
					<select v-model="ctx.runMode.value" class="rounded-xl border-0 bg-surface-4 px-3 py-2">
						<option value="manual">Connect manually</option>
						<option value="app_open">Reconnect when app opens</option>
						<option value="startup">Always prefer this Core</option>
					</select>
				</label>
			</div>

			<div class="rounded-2xl bg-surface-3 p-5">
				<div class="mb-4 flex items-center gap-3">
					<ShieldIcon class="text-brand" />
					<h2 class="m-0 text-xl font-bold text-contrast">Safety</h2>
				</div>
				<label class="flex flex-col gap-2">
					<span class="text-sm font-bold text-contrast">Backup retention</span>
					<StyledInput v-model="ctx.backupRetention.value" type="number" :icon="HistoryIcon" min="1" max="30" />
				</label>
				<div class="mt-4 flex items-center justify-between gap-4">
					<div>
						<div class="font-bold text-contrast">Share anonymous diagnostics</div>
						<p class="m-0 text-sm text-secondary">UI-only placeholder until Core telemetry exists.</p>
					</div>
					<Toggle v-model="ctx.shareTelemetry.value" />
				</div>
			</div>
		</div>

		<label v-if="ctx.mode === 'setup'" class="flex flex-col gap-2 rounded-2xl bg-surface-3 p-5">
			<span class="flex items-center gap-2 text-sm font-bold text-contrast"><FolderOpenIcon /> Install location</span>
			<StyledInput v-model="ctx.installPath.value" />
		</label>
	</div>
</template>
