<template>
	<ServersManageAccessPage :user-profile-link="userProfileLink" />
</template>

<script setup lang="ts">
import { ServersManageAccessPage } from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'

function userProfileLink(username: string) {
	return () => openUrl(`https://modrinth.com/user/${encodeURIComponent(username)}`)
}
</script>
