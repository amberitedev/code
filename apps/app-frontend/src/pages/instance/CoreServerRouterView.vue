<script setup lang="ts">
import {
	CoreServerManageBackupsPage,
	CoreServerManageContentPage,
	CoreServerManageFilesPage,
	CoreServerManageOverviewPage,
} from '@modrinth/ui'
import { computed } from 'vue'

const props = defineProps<{
	tab: 'overview' | 'content' | 'files' | 'backups'
}>()

const component = computed(() => {
	switch (props.tab) {
		case 'content':
			return CoreServerManageContentPage
		case 'files':
			return CoreServerManageFilesPage
		case 'backups':
			return CoreServerManageBackupsPage
		case 'overview':
		default:
			return CoreServerManageOverviewPage
	}
})
</script>

<template>
	<component :is="component" />
</template>
