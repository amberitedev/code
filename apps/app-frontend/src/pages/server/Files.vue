<script setup lang="ts">
import { ServersManageFilesPage } from '@modrinth/ui'
// AMBERITE PATCH: kyros files prefetch removed — no Core file API equivalent yet.
// ServersManageFilesPage will handle its own data fetching internally.
</script>

<template>
	<ServersManageFilesPage />
</template>
