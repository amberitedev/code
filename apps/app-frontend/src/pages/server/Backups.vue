<script setup lang="ts">
import { injectModrinthServerContext, ServersManageBackupsPage } from '@modrinth/ui'

const { isServerRunning } = injectModrinthServerContext()
// AMBERITE PATCH: Archon backup prefetch removed — no Core backup API equivalent yet.
// ServersManageBackupsPage will handle its own data fetching internally.
</script>

<template>
	<ServersManageBackupsPage :is-server-running="isServerRunning" />
</template>
