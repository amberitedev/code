<template>
	<ServersManageRootLayout
		:server-id="serverId"
		:base-path="`/hosting/manage/${serverId}`"
		:reload-page="() => reloadNuxtApp({ path: route.path })"
		:show-copy-id-action="flags.developerMode"
		:show-advanced-debug-info="flags.advancedDebugInfo"
		:navigate-to-servers="() => router.push('/hosting/manage')"
		constrain-width
	>
		<template #default="{ onReinstall, onReinstallFailed }">
			<NuxtPage :route="route" @reinstall="onReinstall" @reinstall-failed="onReinstallFailed" />
		</template>
	</ServersManageRootLayout>
</template>

<script setup lang="ts">
import { ServersManageRootLayout } from '@modrinth/ui'

import { reloadNuxtApp } from '#app'

const flags = useFeatureFlags()
const route = useNativeRoute()
const router = useRouter()
const serverId = route.params.id as string
</script>
