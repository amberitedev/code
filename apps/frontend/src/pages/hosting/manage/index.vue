<script setup lang="ts">
import { ServersManagePageIndex } from '@modrinth/ui'

useHead({
	title: 'Hosting - Modrinth',
})
</script>

<template>
	<ServersManagePageIndex manage-base-path="/hosting/manage" class="max-w-[1280px] py-0" />
</template>
