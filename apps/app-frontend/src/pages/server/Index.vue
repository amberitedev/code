<template>
	<div class="h-full w-full pt-6">
		<div
			v-if="coreError"
			class="flex min-h-[calc(100vh-4rem)] items-center justify-center text-contrast"
		>
			<ErrorInformationCard
				title="Core Unavailable"
				description="Amberite Core is not responding. Make sure Core is running and try again."
				:icon="IssuesIcon"
			/>
		</div>
		<CoreServerManageRootLayout
			v-else
			:server-id="coreInstanceId"
			:nav-href-prefix="`/instance/${encodeURIComponent(profilePath)}`"
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
						query: {
							sid,
							wid: wid ?? undefined,
							from,
							source: 'core',
							back: from ? undefined : `/instance/${encodeURIComponent(profilePath)}/content`,
						},
					})
				}
			"
			:browse-content="
				({ serverId: sid, worldId: wid, type }) => {
					router.push({
						path: `/browse/${type}`,
						query: {
							sid,
							wid: wid ?? undefined,
							source: 'core',
							back: `/instance/${encodeURIComponent(profilePath)}/content`,
						},
					})
				}
			"
		>
			<template #default="{ onReinstall, onReinstallFailed }">
				<RouterView v-slot="{ Component }">
					<template v-if="Component">
						<Suspense :key="route.path">
							<component
								:is="Component"
								@reinstall="onReinstall"
								@reinstall-failed="onReinstallFailed"
							/>
						</Suspense>
					</template>
				</RouterView>
			</template>
		</CoreServerManageRootLayout>
	</div>
</template>

<script setup lang="ts">
import { IssuesIcon } from '@modrinth/assets'
import {
	CoreServerManageRootLayout,
	ErrorInformationCard,
	injectAuth,
	injectCoreInstanceState,
} from '@modrinth/ui'
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { get_user } from '@/helpers/cache'
import { get as getCreds } from '@/helpers/mr_auth'
import { get as getProfile } from '@/helpers/profile'
import type { GameInstance } from '@/helpers/types'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { useTheming } from '@/store/theme'

const route = useRoute()
const router = useRouter()
const auth = injectAuth()
const coreInstances = injectCoreInstanceState()
const themeStore = useTheming()
const breadcrumbs = useBreadcrumbs()

const coreError = ref(false)

const profilePath = computed(() => {
	const id = route.params.id
	return Array.isArray(id) ? id[0] : (id ?? '')
})
const profile = ref<GameInstance | null>(null)
const coreInstanceId = computed(() => profile.value?.core_instance_id ?? '')

async function loadProfile(path = profilePath.value) {
	coreError.value = false
	const nextProfile = await getProfile(path).catch(() => null)
	if (path !== profilePath.value) return

	profile.value = nextProfile
	if (!nextProfile?.core_instance_id) {
		coreError.value = true
		return
	}

	void coreInstances.refresh().catch(() => {
		coreError.value = true
	})

	if (nextProfile.name) {
		breadcrumbs.setName('Server', nextProfile.name)
		breadcrumbs.setContext({
			name: nextProfile.name,
			link: `/instance/${encodeURIComponent(path)}/content`,
		})
	}
}

await loadProfile()
watch(profilePath, (path) => void loadProfile(path))

const authUser = computed(() => {
	const user = auth.user.value
	if (!user?.id) return undefined
	return { id: user.id, username: user.username, email: user.email ?? '', created: user.created }
})

async function resolveViewer(): Promise<{ userId: string | null; userRole: string | null }> {
	const credentials = await getCreds().catch(() => null)
	if (!credentials?.user_id) return { userId: null, userRole: null }
	const user = await get_user(credentials.user_id, 'bypass').catch(() => null)
	return {
		userId: credentials.user_id,
		userRole: (user as { role?: string | null } | null)?.role ?? null,
	}
}
</script>
