<template>
	<CoreServerSettingsPanel v-if="instance.kind === 'server'" :instance="instance" />
	<div v-else-if="instance.kind === 'synced'" class="flex flex-col gap-4">
		<Tabs :value="activeTab" :tabs="tabs" @change="(tab) => (activeTab = toTab(tab.value))" />
		<CoreServerSettingsPanel v-if="activeTab === 'server'" :instance="instance" />
		<ClientSettingsPanel
			v-else
			:instance="instance"
			:offline="offline"
			@refresh="emit('refresh')"
		/>
	</div>
	<ClientSettingsPanel v-else :instance="instance" :offline="offline" @refresh="emit('refresh')" />
</template>

<script setup lang="ts">
import { GameIcon, ServerIcon } from '@modrinth/assets'
import { Tabs, type TabsTab, type TabsValue } from '@modrinth/ui'
import { ref } from 'vue'

import type { GameInstance } from '@/helpers/types'

import ClientSettingsPanel from './ClientSettingsPanel.vue'
import CoreServerSettingsPanel from './CoreServerSettingsPanel.vue'

type SettingsTab = 'client' | 'server'

defineProps<{
	instance: GameInstance
	offline?: boolean
}>()

const emit = defineEmits<{
	refresh: []
}>()

const activeTab = ref<SettingsTab>('client')
const tabs: TabsTab[] = [
	{ value: 'client', label: 'Client settings', icon: GameIcon },
	{ value: 'server', label: 'Server settings', icon: ServerIcon },
]

function toTab(value: TabsValue): SettingsTab {
	return value === 'server' ? 'server' : 'client'
}
</script>
