<script setup lang="ts">
import { ServersManageAccessPage } from '@modrinth/ui'

function userProfileLink(username: string) {
	return `/user/${encodeURIComponent(username)}`
}
</script>

<template>
	<ServersManageAccessPage :user-profile-link="userProfileLink" />
</template>
