<script setup lang="ts">
import { ServerStackIcon } from '@modrinth/assets'

import CoreServers from '@/features/servers/core-servers.vue'
import { useRootBreadcrumb } from '@/shared/breadcrumbs'

useRootBreadcrumb({
	slot: 'root',
	id: 'servers',
	label: 'Servers',
	to: '/hosting/manage/',
	visual: { type: 'icon', component: ServerStackIcon },
})
</script>

<template>
	<CoreServers />
</template>
