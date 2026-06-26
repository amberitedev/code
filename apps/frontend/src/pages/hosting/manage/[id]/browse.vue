<script setup lang="ts">
import { ServersManageBrowsePage } from '@modrinth/ui'

const route = useNativeRoute()
const backPath = computed(() => `/hosting/manage/${encodeURIComponent(route.params.id as string)}/content`)

useHead({
	title: 'Add content - Modrinth',
})

definePageMeta({
	middleware: 'auth',
})
</script>

<template>
	<ServersManageBrowsePage :back-path="backPath" variant="web" />
</template>
