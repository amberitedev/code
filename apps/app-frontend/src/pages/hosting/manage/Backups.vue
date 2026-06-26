<template>
	<ServersManageBackupsPage :is-server-running="isServerRunning" />
</template>

<script setup lang="ts">
import { injectModrinthServerContext, ServersManageBackupsPage } from '@modrinth/ui'

const { isServerRunning } = injectModrinthServerContext()
</script>
