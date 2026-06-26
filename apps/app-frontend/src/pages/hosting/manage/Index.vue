<template>
	<div class="h-full w-full">
		<ServersManageRootLayout
			:server-id="serverId"
			:base-path="`/hosting/manage/${serverId}`"
			:reload-page="() => router.go(0)"
			:resolve-viewer="resolveViewer"
			:show-copy-id-action="themeStore.devMode"
			:auth-user="authUser"
			:navigate-to-servers="() => router.push('/hosting/manage')"
		>
			<template #default="{ onReinstall, onReinstallFailed }">
				<RouterView v-slot="{ Component }">
					<template v-if="Component">
						<Suspense>
							<component
								:is="Component"
								@reinstall="onReinstall"
								@reinstall-failed="onReinstallFailed"
							/>
						</Suspense>
					</template>
				</RouterView>
			</template>
		</ServersManageRootLayout>
	</div>
</template>

<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { injectAuth, ServersManageRootLayout } from '@modrinth/ui'
import { computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { get_user } from '@/helpers/cache'
import { get as getCreds } from '@/helpers/mr_auth'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { useTheming } from '@/store/theme'

const route = useRoute()
const router = useRouter()
const auth = injectAuth()
const themeStore = useTheming()
const breadcrumbs = useBreadcrumbs()

const serverId = computed(() => {
	const rawId = route.params.id
	return Array.isArray(rawId) ? rawId[0] : (rawId ?? '')
})

watch(
	serverId,
	(id) => {
		if (!id) return
		breadcrumbs.setName('Server', id)
		breadcrumbs.setContext({
			name: id,
			link: `/hosting/manage/${id}`,
		})
	},
	{ immediate: true },
)

watch(
	() => auth.user.value,
	(user, previousUser) => {
		if (user || !previousUser) return
		if (route.path === '/hosting/manage' || route.path === '/hosting/manage/') return
		void router.replace('/hosting/manage')
	},
)

const authUser = computed(() => {
	const user = auth.user.value
	if (!user?.id) return undefined
	return {
		id: user.id,
		username: user.username,
		email: user.email ?? '',
		created: user.created,
	}
})

async function resolveViewer(): Promise<{ userId: string | null; userRole: string | null }> {
	const credentials = await getCreds().catch(() => null)
	if (!credentials?.user_id) return { userId: null, userRole: null }

	const user = await get_user(credentials.user_id, 'bypass').catch(() => null)
	const typedUser = user as Labrinth.Users.v2.User | null
	return {
		userId: credentials.user_id,
		userRole: typedUser?.role ?? null,
	}
}
</script>
