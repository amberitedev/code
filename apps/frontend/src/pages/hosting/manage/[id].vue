<template>
	<ServersManageRootLayout
		:server-id="serverId"
		:base-path="`/hosting/manage/${serverId}`"
		:reload-page="() => reloadNuxtApp({ path: route.path })"
		:resolve-viewer="resolveViewer"
		:show-copy-id-action="flags.developerMode"
		:show-advanced-debug-info="flags.advancedDebugInfo"
		:auth-user="authUser"
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
const auth = (await useAuth()) as unknown as {
	value: { user: { id: string; username: string; email: string; created: string; role?: string } }
}
const authUser = auth.value?.user
	? {
			id: auth.value.user.id,
			username: auth.value.user.username,
			email: auth.value.user.email,
			created: auth.value.user.created,
		}
	: undefined

async function resolveViewer(): Promise<{ userId: string | null; userRole: string | null }> {
	return {
		userId: auth.value?.user?.id ?? null,
		userRole: auth.value?.user?.role ?? null,
	}
}

definePageMeta({
	middleware: 'auth',
})
</script>
