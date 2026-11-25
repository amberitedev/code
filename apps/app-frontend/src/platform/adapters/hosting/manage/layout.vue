<template>
	<div class="w-full pt-6" :class="isOverviewRoute ? 'flex min-h-full flex-col' : 'h-full'">
		<div
			v-if="coreError"
			class="flex min-h-[calc(100vh-4rem)] items-center justify-center text-contrast"
		>
			<ErrorInformationCard
				v-bind="coreErrorCopy"
				:icon="TriangleAlertIcon"
				icon-color="red"
				:action="{ label: 'Try again', onClick: () => void refetchCore(), color: 'brand' }"
			/>
		</div>
		<ServersManageRootLayout
			v-else-if="coreId"
			:key="serverId"
			:server-id="serverId"
			:core-id="coreId"
			:layout-mode="isOverviewRoute ? 'fill' : 'page'"
			:reload-page="() => router.go(0)"
			:additional-tabs="historyTabs"
			:site-url="config.accountSiteUrl"
			:navigate-to-servers="() => router.push('/hosting/manage')"
			:browse-modpacks="
				({ serverId: sid, from }) => {
					router.push({ path: '/browse/modpack', query: { sid, from } })
				}
			"
			:browse-content="
				({ serverId: sid, type }) => {
					router.push({ path: `/browse/${type}`, query: { sid } })
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
import { core, type Server } from '@amberite/contracts/core'
import { ApiCallError } from '@modrinth/api-client'
import { HistoryIcon, ServerStackIcon, TriangleAlertIcon } from '@modrinth/assets'
import {
	commonMessages,
	ErrorInformationCard,
	injectAuth,
	injectModrinthClient,
	serverKeys,
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
		.getQueryData<{ servers: Server[] }>(serverKeys.list())
		?.servers.find((server) => server.server_id === id)?.name
}

// The panel talks to the server's Core, so it opens once the Core is known.
const {
	data: coreId,
	error: coreError,
	refetch: refetchCore,
} = useQuery({
	queryKey: computed(() => ['servers', 'core', serverId.value]),
	queryFn: async () => (await client.cores.coreOf(serverId.value)).id,
	enabled: computed(() => Boolean(serverId.value)),
	// No Core answered, or none has the server: say so at once.
	retry: false,
	staleTime: Infinity,
})
const coreErrorCopy = computed(() =>
	coreError.value instanceof ApiCallError && coreError.value.code === 'not_found'
		? { title: 'Server not found', description: 'None of your Cores has this server.' }
		: {
				title: "Can't reach your Core",
				description: "Check that your Core is running and that you're online, then try again.",
			},
)

const { data: serverData } = useQuery({
	queryKey: computed(() => serverKeys.detail(serverId.value)),
	queryFn: async () =>
		(await client.cores.serverRoutes(serverId.value)).call(core.GetServer, {
			params: { id: serverId.value },
		}),
	enabled: computed(() => Boolean(serverId.value)),
	// The panel's own query of this server says when its Core is offline; retrying only delays it.
	retry: false,
	placeholderData: () =>
		queryClient
			.getQueryData<{ servers: Server[] }>(serverKeys.list())
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
