<template>
	<div v-if="instance.kind === 'synced'" class="flex flex-col gap-4">
		<Tabs :value="activeTab" :tabs="tabs" @change="(tab) => (activeTab = toTab(tab.value))" />
		<CoreServerManageContentPage v-if="activeTab === 'server'" />
		<Mods v-else v-bind="props" @play="emit('play')" @stop="emit('stop')" />
	</div>
	<CoreServerManageContentPage v-else-if="instance.kind === 'server'" />
	<Mods v-else v-bind="props" @play="emit('play')" @stop="emit('stop')" />
</template>

<script setup lang="ts">
import { BoxesIcon, ServerIcon } from '@modrinth/assets'
import { CoreServerManageContentPage, Tabs, type TabsTab, type TabsValue } from '@modrinth/ui'
import { ref } from 'vue'

import type { InstanceContentData } from '@/helpers/instance-content'
import type { GameInstance } from '@/helpers/types'

import Mods from './Mods.vue'

type ContentTab = 'client' | 'server'

const props = defineProps<{
	instance: GameInstance
	options?: unknown
	offline?: boolean
	playing?: boolean
	installed?: boolean
	isServerInstance?: boolean
	openSettings?: () => void
	preloadedContent?: InstanceContentData | null
}>()

const emit = defineEmits<{
	play: []
	stop: []
}>()

const activeTab = ref<ContentTab>('client')
const tabs: TabsTab[] = [
	{ value: 'client', label: 'Client content', icon: BoxesIcon },
	{ value: 'server', label: 'Server content', icon: ServerIcon },
]

function toTab(value: TabsValue): ContentTab {
	return value === 'server' ? 'server' : 'client'
}
</script>
