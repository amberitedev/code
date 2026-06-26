<template>
	<ServersManageRootLayout
		:server-id="serverId"
		:base-path="basePath"
		servers-path="/library/servers"
		:navigate-to-servers="backToServerLibrary"
		show-copy-id-action
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

import type { GameInstance } from '@/helpers/types'
import { useBreadcrumbs } from '@/store/breadcrumbs'

const route = useRoute()
const router = useRouter()
const breadcrumbs = useBreadcrumbs()
const props = defineProps<{
	profile?: GameInstance | null
}>()

const serverId = computed(() => props.profile?.core_instance_id ?? (route.params.id as string))
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
	serverId,
	(id) => {
		if (!id) return
		breadcrumbs.setName('Instance', id.length > 40 ? id.substring(0, 40) + '...' : id)
		breadcrumbs.setContext({
			name: id,
			link: route.path,
			query: route.query,
		})
	},
	{ immediate: true },
)
</script>
