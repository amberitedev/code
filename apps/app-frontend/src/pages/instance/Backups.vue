<template>
	<CoreServerManageBackupsPage v-if="instance.kind === 'server' || instance.kind === 'synced'" />
	<EmptyState
		v-else
		type="empty-inbox"
		heading="No server backups"
		description="Backups are available for Core-managed servers and synced instances."
	/>
</template>

<script setup lang="ts">
import { CoreServerManageBackupsPage, EmptyState } from '@modrinth/ui'

import type { GameInstance } from '@/helpers/types'

defineProps<{
	instance: GameInstance
}>()
</script>
