<template>
	<div class="flex flex-col gap-6">
		<InstanceSettingsProvider :instance="instance" :offline="offline" @unlinked="emit('refresh')">
			<GeneralSettings />
			<InstallationSettings />
			<WindowSettings />
			<JavaSettings />
			<HooksSettings />
		</InstanceSettingsProvider>
	</div>
</template>

<script setup lang="ts">
import GeneralSettings from '@/components/ui/instance_settings/GeneralSettings.vue'
import HooksSettings from '@/components/ui/instance_settings/HooksSettings.vue'
import InstallationSettings from '@/components/ui/instance_settings/InstallationSettings.vue'
import JavaSettings from '@/components/ui/instance_settings/JavaSettings.vue'
import WindowSettings from '@/components/ui/instance_settings/WindowSettings.vue'
import type { GameInstance } from '@/helpers/types'

import InstanceSettingsProvider from './InstanceSettingsProvider.vue'

defineProps<{
	instance: GameInstance
	offline?: boolean
}>()

const emit = defineEmits<{
	refresh: []
}>()
</script>
