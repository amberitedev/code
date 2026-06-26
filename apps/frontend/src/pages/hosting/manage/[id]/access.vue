<script setup lang="ts">
import { injectModrinthServerContext, ServersManageAccessPage } from '@modrinth/ui'

const { server } = injectModrinthServerContext()
const flags = useFeatureFlags()

useHead({
	title: computed(() => `Access - ${server.value?.name ?? 'Server'} - Modrinth`),
})
</script>

<template>
	<ServersManageAccessPage :show-audit-log-instances="flags.showHostingAccessInstanceAuditLog" />
</template>
