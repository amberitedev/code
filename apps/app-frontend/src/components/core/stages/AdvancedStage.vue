<script setup lang="ts">
import { DropdownSelect } from '@modrinth/ui'
import { computed } from 'vue'

import {
	type CoreStartupRunMode,
	injectCoreOnboardingContext,
} from '../core-onboarding-context'

const ctx = injectCoreOnboardingContext()
const startupRunModes: CoreStartupRunMode[] = ['manual', 'app_open', 'startup']
const selectedStartupRunModeDescription = computed(() =>
	startupRunModeDescription(ctx.startupRunMode.value),
)

function startupRunModeLabel(mode: CoreStartupRunMode) {
	switch (mode) {
		case 'manual':
			return 'Manual'
		case 'app_open':
			return 'When Amberite opens'
		case 'startup':
			return 'On system startup'
	}
}

function startupRunModeDescription(mode: CoreStartupRunMode) {
	switch (mode) {
		case 'manual':
			return 'Start the Core yourself when you need it.'
		case 'app_open':
			return 'Start the Core automatically when the desktop app opens.'
		case 'startup':
			return 'Start the Core when the computer starts, before the app is opened.'
	}
}
</script>

<template>
	<div class="flex min-h-[26rem] flex-col gap-6">
		<section class="flex flex-col gap-2">
			<h2 class="m-0 text-xl font-bold text-contrast">Advanced settings</h2>
			<p class="m-0 text-base leading-6 text-secondary">
				Choose how the Core should run after setup.
			</p>
		</section>

		<section v-if="ctx.flow.value === 'create'" class="flex w-full flex-1">
			<div
				class="grid w-full flex-1 grid-cols-1 gap-3 rounded-xl bg-surface-2 p-4 md:grid-cols-[minmax(0,1fr)_20rem] md:items-center"
			>
				<div class="flex min-w-0 flex-col gap-1">
					<span class="font-semibold text-contrast">Core startup</span>
					<span class="text-sm leading-5 text-secondary">
						{{ selectedStartupRunModeDescription }}
					</span>
				</div>
				<DropdownSelect
					v-model="ctx.startupRunMode.value"
					name="core-startup-run-mode"
					:options="startupRunModes"
					:display-name="startupRunModeLabel"
					style="width: 100%"
				/>
			</div>
		</section>

		<section v-else class="flex-1 rounded-xl bg-surface-2 p-4">
			<p class="m-0 text-sm leading-5 text-secondary">
				This Core is already set up. Its startup behavior stays with the existing Core owner and
				can be changed from Core settings after setup.
			</p>
		</section>

		<p v-if="ctx.error.value" class="m-0 text-sm text-red" role="alert">
			{{ ctx.error.value }}
		</p>
	</div>
</template>
