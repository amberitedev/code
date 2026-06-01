<template>
	<BrowsePageLayout />
</template>

<script setup lang="ts">
import { BrowsePageLayout } from '@modrinth/ui'
import { computed } from 'vue'

import { useCoreBrowseManager } from './use-core-browse-manager'

const props = defineProps<{ installedProjectIds: string[] }>()
const emit = defineEmits<{ back: []; installed: [] }>()

useCoreBrowseManager({
	installedProjectIds: computed(() => new Set(props.installedProjectIds)),
	onBack: () => emit('back'),
	onInstalled: () => emit('installed'),
})
</script>
