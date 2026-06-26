<template>
	<ServersManageBrowsePage :back-path="backPath" />
</template>

<script setup lang="ts">
import { ServersManageBrowsePage } from '@modrinth/ui'
import { computed } from 'vue'
import { useRoute } from 'vue-router'

const route = useRoute()
const backPath = computed(() => `/hosting/manage/${encodeURIComponent(route.params.id as string)}/content`)
</script>
