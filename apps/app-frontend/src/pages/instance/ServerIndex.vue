<template>
	<ServersManageRootLayout
		:server-id="serverId"
		:base-path="basePath"
		servers-path="/library/servers"
		:navigate-to-servers="backToServerLibrary"
		show-copy-id-action
		copy-id-label="Copy path"
	>
		<component :is="activePage" v-bind="activePageProps" />
	</ServersManageRootLayout>
</template>

<script setup lang="ts">
import {
	ServersManageAccessPage,
	ServersManageBackupsPage,
	ServersManageBrowsePage,
	ServersManageContentPage,
	ServersManageFilesPage,
	ServersManageOverviewPage,
	ServersManageRootLayout,
} from '@modrinth/ui'
import { computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useCoreInstances } from '@/composables/useCoreInstances'
import type { GameInstance } from '@/helpers/types'
import { useBreadcrumbs } from '@/store/breadcrumbs'

const route = useRoute()
const router = useRouter()
const breadcrumbs = useBreadcrumbs()
const props = defineProps<{
	profile?: GameInstance | null
}>()

const { instances: coreInstances } = useCoreInstances()
const serverId = computed(() => route.params.id as string)
const coreInstance = computed(() =>
	[...coreInstances.value.values()].find((instance) => instance.path === serverId.value),
)
const basePath = computed(() => `/instance/${encodeURIComponent(route.params.id as string)}`)
const activePage = computed(() => {
	if (route.path.endsWith('/content')) return ServersManageContentPage
	if (route.path.endsWith('/files')) return ServersManageFilesPage
	if (route.path.endsWith('/access')) return ServersManageAccessPage
	if (route.path.endsWith('/backups')) return ServersManageBackupsPage
	if (route.path.endsWith('/browse')) return ServersManageBrowsePage
	return ServersManageOverviewPage
})
const activePageProps = computed(() => {
	if (route.path.endsWith('/browse')) return { backPath: `${basePath.value}/content` }
	return {}
})

async function backToServerLibrary() {
	await router.replace('/library/servers')
}

watch(
	[serverId, coreInstance],
	([id, instance]) => {
		if (!id) return
		const name = instance?.name ?? props.profile?.name ?? id
		breadcrumbs.setName('Instance', name.length > 40 ? name.substring(0, 40) + '...' : name)
		breadcrumbs.setContext({
			name,
			link: route.path,
			query: route.query,
		})
	},
	{ immediate: true },
)
</script>
