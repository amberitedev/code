<script setup lang="ts">
import { ServerStackIcon } from '@modrinth/assets'
import { ButtonStyled, StyledInput, Toggle } from '@modrinth/ui'
import { onMounted, ref } from 'vue'

import {
	type CoreAppSettings,
	getCoreAppSettings,
	setCoreAppSettings,
} from '@/helpers/amberite-core'

const settings = ref<CoreAppSettings>({
	core_url: null,
	display_name: null,
	auto_launch_core: false,
})
const loading = ref(true)
const error = ref<string | null>(null)
const saved = ref(false)

onMounted(async () => {
	settings.value = await getCoreAppSettings().catch((err) => {
		error.value = err instanceof Error ? err.message : String(err)
		return settings.value
	})
	loading.value = false
})

async function saveSettings() {
	if (loading.value) return
	await setCoreAppSettings(settings.value)
	saved.value = true
	setTimeout(() => (saved.value = false), 1800)
}
</script>

<template>
	<div class="flex flex-col gap-6">
		<div class="rounded-2xl border border-solid border-button-border bg-bg p-4">
			<div class="flex items-center gap-3">
				<ServerStackIcon class="text-brand" />
				<div>
					<h2 class="m-0 text-lg font-semibold text-contrast">Amberite Core connection</h2>
					<p class="m-0 mt-1 text-sm text-secondary">
						Local app preferences for the Core dashboard.
					</p>
				</div>
			</div>
		</div>

		<div class="flex flex-col gap-2.5">
			<h3 class="m-0 text-lg font-semibold text-contrast">Core URL</h3>
			<p v-if="loading" class="m-0 text-sm text-secondary">Loading Core settings...</p>
			<p v-if="error" class="m-0 text-sm font-semibold text-red">{{ error }}</p>
			<StyledInput
				v-model="settings.core_url"
				placeholder="http://localhost:16662"
				wrapper-class="w-full"
			/>
			<p class="m-0 text-sm text-secondary">Leave empty to use the default local Core URL.</p>
		</div>

		<div class="flex flex-col gap-2.5">
			<h3 class="m-0 text-lg font-semibold text-contrast">Display name</h3>
			<StyledInput
				v-model="settings.display_name"
				placeholder="How friends see you"
				wrapper-class="w-full"
			/>
		</div>

		<div class="flex items-center justify-between gap-4">
			<div>
				<h3 class="m-0 text-lg font-semibold text-contrast">Auto-launch local Core</h3>
				<p class="m-0 mt-1 text-sm text-secondary">
					The process launcher is scaffolded; this preference is stored for the install flow.
				</p>
			</div>
			<Toggle id="core-auto-launch" v-model="settings.auto_launch_core" />
		</div>

		<ButtonStyled color="brand">
			<button :disabled="loading" @click="saveSettings">Save Core settings</button>
		</ButtonStyled>
		<p v-if="saved" class="m-0 text-sm font-semibold text-brand">Saved.</p>
	</div>
</template>
