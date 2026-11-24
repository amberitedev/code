<template>
	<div class="w-full pt-6" :class="isOverviewRoute ? 'flex min-h-full flex-col' : 'h-full'">
		<ServersManageRootLayout
			:server-id="serverId"
			:layout-mode="isOverviewRoute ? 'fill' : 'page'"
			:reload-page="() => router.go(0)"
			:additional-tabs="historyTabs"
			:site-url="config.accountSiteUrl"
			:navigate-to-servers="() => router.push('/hosting/manage')"
			:browse-modpacks="
				({ serverId: sid, worldId: wid, from }) => {
					router.push({
						path: '/browse/modpack',
						query: { sid, wid: wid ?? undefined, from },
					})
				}
			"
			:browse-content="
				({ serverId: sid, worldId: wid, type }) => {
					router.push({
						path: `/browse/${type}`,
						query: { sid, wid: wid ?? undefined },
					})
				}
			"
		>
			<template #default>
				<RouterView v-slot="{ Component }">
					<template v-if="Component">
						<Suspense>
							<component :is="Component" />
						</Suspense>
					</template>
				</RouterView>
			</template>
		</ServersManageRootLayout>
	</div>
</template>

<script setup lang="ts">
import type { Archon } from '@modrinth/api-client'
import { HistoryIcon, ServerStackIcon } from '@modrinth/assets'
import {
	commonMessages,
	injectAuth,
	injectModrinthClient,
	ServersManageRootLayout,
	useVIntl,
} from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref, shallowRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { config } from '@/config'
import { useServerHistory } from '@/features/share-history/use-server-history'
import { provideBreadcrumbParent, useBreadcrumb, useRootBreadcrumb } from '@/shared/breadcrumbs'
import { useCachedServerIcon } from '@/shared/composables/use-cached-server-icon'

const route = useRoute()
const router = useRouter()
const auth = injectAuth()
const client = injectModrinthClient()
const queryClient = useQueryClient()
const { formatMessage } = useVIntl()

const serverRoute = shallowRef(router.currentRoute.value)
watch(
	router.currentRoute,
	(nextRoute) => {
		if (nextRoute.matched.some((record) => record.name === 'ServerManage')) {
			serverRoute.value = nextRoute
		}
	},
	{ flush: 'sync' },
)

const isOverviewRoute = computed(() => serverRoute.value.name === 'ServerManageOverview')

const serverId = computed(() => {
	const rawId = serverRoute.value.params.id
	return Array.isArray(rawId) ? (rawId[0] ?? '') : (rawId ?? '')
})

// History lives with the share's source, and only its owner sees it (`useServerHistory`).
const historySource = useServerHistory(serverId)
const historyTabs = computed(() =>
	historySource.value
		? [
				{
					label: 'History',
					href: `/hosting/manage/${serverId.value}/history`,
					icon: HistoryIcon,
				},
			]
		: [],
)

function getCachedServerName(id: string): string | undefined {
	return queryClient
		.getQueryData<Archon.Servers.v0.ServerGetResponse>(['servers'])
		?.servers.find((server) => server.server_id === id)?.name
}

const { data: serverData } = useQuery({
	queryKey: computed(() => ['servers', 'detail', serverId.value]),
	queryFn: () => client.archon.servers_v0.get(serverId.value),
	enabled: computed(() => Boolean(serverId.value)),
	// The panel's own query of this server says when its Core is offline; retrying only delays it.
	retry: false,
	placeholderData: () =>
		queryClient
			.getQueryData<Archon.Servers.v0.ServerGetResponse>(['servers'])
			?.servers.find((server) => server.server_id === serverId.value),
	staleTime: 30_000,
})

const breadcrumbServerId = ref(serverId.value)
const breadcrumbLabel = ref(
	getCachedServerName(serverId.value) ?? formatMessage(commonMessages.loadingLabel),
)
watch(
	serverId,
	(value) => {
		if (!route.path.startsWith('/hosting/manage/') || route.name === 'Servers') return
		breadcrumbServerId.value = value
		breadcrumbLabel.value = getCachedServerName(value) ?? formatMessage(commonMessages.loadingLabel)
	},
	{ flush: 'sync' },
)
watch(
	serverData,
	(server) => {
		if (!route.path.startsWith('/hosting/manage/') || !server?.name) return
		breadcrumbLabel.value = server.name
	},
	{ immediate: true },
)

const serverIcon = useCachedServerIcon(breadcrumbServerId)
const hostingBreadcrumb = useRootBreadcrumb({
	slot: 'root',
	id: 'servers',
	label: 'Servers',
	to: '/hosting/manage/',
	visual: { type: 'icon', component: ServerStackIcon },
})
const serverBreadcrumb = useBreadcrumb(
	{
		slot: 'server',
		id: () => `server:${breadcrumbServerId.value}`,
		label: breadcrumbLabel,
		visual: () => ({
			type: 'image',
			src: serverIcon.value,
			alt: breadcrumbLabel.value,
		}),
		to: () => `/hosting/manage/${encodeURIComponent(breadcrumbServerId.value)}`,
	},
	{ parent: hostingBreadcrumb },
)
provideBreadcrumbParent(serverBreadcrumb)

watch(
	() => auth.user.value,
	(user, previousUser) => {
		if (user || !previousUser) return
		if (route.path === '/hosting/manage' || route.path === '/hosting/manage/') return
		void router.replace('/hosting/manage')
	},
)
</script>
