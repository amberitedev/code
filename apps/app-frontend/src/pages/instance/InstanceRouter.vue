<template>
	<SyncedIndex v-if="isSyncedProfile" />
	<ServerIndex v-else-if="isServerProfile" :profile="profile" />
	<ClientIndex v-else />
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { profile_listener } from '@/helpers/events'
import { get, remove as removeProfile } from '@/helpers/profile'
import type { GameInstance } from '@/helpers/types'
import { useBreadcrumbs } from '@/store/breadcrumbs'

import ClientIndex from './Index.vue'

const ServerIndex = defineAsyncComponent(() => import('./ServerIndex.vue'))
const SyncedIndex = defineAsyncComponent(() => import('./synced/SyncedIndex.vue'))

const route = useRoute()
const router = useRouter()
const breadcrumbs = useBreadcrumbs()
const profile = ref<GameInstance | null>(null)
const isServerProfile = computed(() => profile.value?.profile_type === 'server')
const isSyncedProfile = computed(() => profile.value?.profile_type === 'synced')

function setInstanceBreadcrumb(name: string) {
	breadcrumbs.setName('Instance', name.length > 40 ? name.substring(0, 40) + '...' : name)
	breadcrumbs.setContext({
		name,
		link: route.path,
		query: route.query,
	})
}

async function refreshProfile() {
	const routeId = route.params.id as string
	let p: GameInstance | null = null
	try {
		p = await get(routeId)
	} catch {
		p = null
	}

	if (p?.profile_type === 'server') {
		void removeProfile(p.path).catch(() => undefined)
		p = null
	}

	if (!p) {
		if (isUuid(routeId)) {
			await router.replace('/library/servers')
			return
		}
		setInstanceBreadcrumb(routeId)
		profile.value = { profile_type: 'server', path: routeId } as GameInstance
		return
	}

	setInstanceBreadcrumb(p.name)
	profile.value = p
	if (profile.value?.profile_type === 'client') return

	if (route.name === 'Logs' || route.name === 'InstanceWorlds' || route.name === 'ModsFilter') {
		await router.replace({
			path: `/instance/${encodeURIComponent(route.params.id as string)}`,
			query: route.query,
		})
	}
}

function isUuid(value: string): boolean {
	return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
		value,
	)
}

await refreshProfile()

watch(
	() => route.params.id,
	() => {
		void refreshProfile()
	},
)

const unlistenProfiles = await profile_listener(
	(event: { profile_path_id: string; event: string }) => {
		if (event.profile_path_id !== route.params.id) return
		if (event.event === 'removed') return
		void refreshProfile()
	},
)

onUnmounted(() => {
	unlistenProfiles()
})
</script>
