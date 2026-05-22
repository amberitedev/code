<template>
	<div v-if="instance.kind === 'synced'" class="flex flex-col gap-4">
		<Tabs :value="activeTab" :tabs="tabs" @change="(tab) => (activeTab = toTab(tab.value))" />
		<CoreServerManageFilesPage v-if="activeTab === 'server'" />
		<Files v-else v-bind="props" @play="emit('play')" @stop="emit('stop')" />
	</div>
	<CoreServerManageFilesPage v-else-if="instance.kind === 'server'" />
	<Files v-else v-bind="props" @play="emit('play')" @stop="emit('stop')" />
</template>

<script setup lang="ts">
import { FolderOpenIcon, ServerIcon } from '@modrinth/assets'
import { CoreServerManageFilesPage, Tabs, type TabsTab, type TabsValue } from '@modrinth/ui'
import { ref } from 'vue'

import type { GameInstance } from '@/helpers/types'

import Files from './Files.vue'

type FilesTab = 'client' | 'server'

const props = defineProps<{
	instance: GameInstance
	options: unknown
	offline: boolean
	playing: boolean
	installed: boolean
	isServerInstance: boolean
	openSettings?: () => void
}>()

const emit = defineEmits<{
	play: []
	stop: []
}>()

const activeTab = ref<FilesTab>('client')
const tabs: TabsTab[] = [
	{ value: 'client', label: 'Client files', icon: FolderOpenIcon },
	{ value: 'server', label: 'Server files', icon: ServerIcon },
]

function toTab(value: TabsValue): FilesTab {
	return value === 'server' ? 'server' : 'client'
}
</script>
