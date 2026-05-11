<template>
	<div class="h-full w-full pt-6">
		<ServersManageRootLayout
			:server-id="coreInstanceId"
			:reload-page="() => router.go(0)"
			:resolve-viewer="resolveViewer"
			:show-copy-id-action="themeStore.devMode"
			:auth-user="authUser"
			:navigate-to-billing="() => {}"
			:navigate-to-servers="() => router.push('/hosting/manage')"
		:browse-modpacks="
			({ serverId: sid, worldId: wid, from }) => {
				router.push({
					path: '/browse/modpack',
					query: { sid, wid: wid ?? undefined, from, back: from ? undefined : `/server/${encodeURIComponent(profilePath)}/content` },
				})
			}
		"
		:browse-content="
			({ serverId: sid, worldId: wid, type }) => {
				router.push({
					path: `/browse/${type}`,
					query: { sid, wid: wid ?? undefined, back: `/server/${encodeURIComponent(profilePath)}/content` },
				})
			}
		"
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
import { CoreApiClient } from '@amberite/core-client'
import { injectAuth, provideCoreClient, ServersManageRootLayout } from '@modrinth/ui'
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { get_user } from '@/helpers/cache'
import { core_get_url } from '@/helpers/core'
import { get as getCreds } from '@/helpers/mr_auth'
import { get as getProfile } from '@/helpers/profile'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { useTheming } from '@/store/theme'

const route = useRoute()
const router = useRouter()
const auth = injectAuth()
const themeStore = useTheming()
const breadcrumbs = useBreadcrumbs()

const profilePath = route.params.id as string
const profile = await getProfile(profilePath).catch(() => null)
const coreInstanceId = profile?.core_instance_id ?? profilePath

if (profile?.name) {
	breadcrumbs.setName('Server', profile.name)
	breadcrumbs.setContext({ name: profile.name, link: `/server/${encodeURIComponent(profilePath)}/content` })
}

const baseUrl = await core_get_url()
provideCoreClient(new CoreApiClient(baseUrl))

const authUser = computed(() => {
	const user = auth.user.value
	if (!user?.id) return undefined
	return { id: user.id, username: user.username, email: user.email ?? '', created: user.created }
})

async function resolveViewer(): Promise<{ userId: string | null; userRole: string | null }> {
	const credentials = await getCreds().catch(() => null)
	if (!credentials?.user_id) return { userId: null, userRole: null }
	const user = await get_user(credentials.user_id, 'bypass').catch(() => null)
	return { userId: credentials.user_id, userRole: (user as { role?: string | null } | null)?.role ?? null }
}
</script>
