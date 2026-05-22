<template>
	<CoreServerManageOverviewPage v-if="instance.kind === 'server'" />
	<div v-else-if="instance.kind === 'synced'" class="flex flex-col gap-4">
		<Tabs :value="activeTab" :tabs="tabs" @change="(tab) => (activeTab = toTab(tab.value))" />
		<CoreServerManageOverviewPage v-if="activeTab === 'server'" />
		<ClientOverview v-else :instance="instance" :playing="playing" :installed="installed" />
	</div>
	<ClientOverview v-else :instance="instance" :playing="playing" :installed="installed" />
</template>

<script setup lang="ts">
import { GameIcon, ServerIcon } from '@modrinth/assets'
import { CoreServerManageOverviewPage, Tabs, type TabsTab, type TabsValue } from '@modrinth/ui'
import { ref } from 'vue'

import type { GameInstance } from '@/helpers/types'

import ClientOverview from './ClientOverview.vue'

type OverviewTab = 'client' | 'server'

defineProps<{
	instance: GameInstance
	playing: boolean
	installed: boolean
}>()

const activeTab = ref<OverviewTab>('client')
const tabs: TabsTab[] = [
	{ value: 'client', label: 'Client overview', icon: GameIcon },
	{ value: 'server', label: 'Server overview', icon: ServerIcon },
]

function toTab(value: TabsValue): OverviewTab {
	return value === 'server' ? 'server' : 'client'
}
</script>
