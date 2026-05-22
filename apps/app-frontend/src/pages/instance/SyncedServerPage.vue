<template>
	<div class="flex flex-col gap-4">
		<Tabs :value="activeTab" :tabs="tabs" @change="setActiveTab" />
		<div class="min-h-0">
			<CoreServerRouterView :tab="activeTab" />
		</div>
	</div>
</template>

<script setup lang="ts">
import { BoxesIcon, DatabaseBackupIcon, FolderOpenIcon, LayoutTemplateIcon } from '@modrinth/assets'
import { Tabs, type TabsTab, type TabsValue } from '@modrinth/ui'
import { computed } from 'vue'

import CoreServerRouterView from './CoreServerRouterView.vue'

const routeTab = defineModel<'overview' | 'content' | 'files' | 'backups'>('tab', {
	default: 'overview',
})

const activeTab = computed(() => routeTab.value)

const tabs: TabsTab[] = [
	{ value: 'overview', label: 'Overview', icon: LayoutTemplateIcon },
	{ value: 'content', label: 'Content', icon: BoxesIcon },
	{ value: 'files', label: 'Files', icon: FolderOpenIcon },
	{ value: 'backups', label: 'Backups', icon: DatabaseBackupIcon },
]

function setActiveTab(tab: TabsTab) {
	routeTab.value = toServerTab(tab.value)
}

function toServerTab(value: TabsValue) {
	return value as 'overview' | 'content' | 'files' | 'backups'
}
</script>
