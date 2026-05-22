<template>
	<div v-if="instance.kind === 'synced'" class="flex h-full flex-col gap-4">
		<Tabs :value="activeTab" :tabs="tabs" @change="(tab) => (activeTab = toTab(tab.value))" />
		<ServerConsole v-if="activeTab === 'server'" />
		<Logs v-else v-bind="props" @play="emit('play')" @stop="emit('stop')" />
	</div>
	<ServerConsole v-else-if="instance.kind === 'server'" />
	<Logs v-else v-bind="props" @play="emit('play')" @stop="emit('stop')" />
</template>

<script setup lang="ts">
import { ServerIcon, TerminalSquareIcon } from '@modrinth/assets'
import { Tabs, type TabsTab, type TabsValue } from '@modrinth/ui'
import { ref } from 'vue'

import type { GameInstance } from '@/helpers/types'
import ServerConsole from '@/pages/server/Console.vue'

import Logs from './Logs.vue'

type LogsTab = 'client' | 'server'

const props = defineProps<{
	instance: GameInstance
	options?: unknown
	offline?: boolean
	playing?: boolean
	installed?: boolean
}>()

const emit = defineEmits<{
	play: []
	stop: []
}>()

const activeTab = ref<LogsTab>('client')
const tabs: TabsTab[] = [
	{ value: 'client', label: 'Client logs', icon: TerminalSquareIcon },
	{ value: 'server', label: 'Server logs', icon: ServerIcon },
]

function toTab(value: TabsValue): LogsTab {
	return value === 'server' ? 'server' : 'client'
}
</script>
