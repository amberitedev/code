<template>
	<SyncedIndex v-if="isSyncedProfile" />
	<ServerIndex v-else-if="isServerProfile" />
	<ClientIndex v-else />
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { profile_listener } from '@/helpers/events'
import { get } from '@/helpers/profile'
import type { GameInstance } from '@/helpers/types'

import ClientIndex from './Index.vue'

const ServerIndex = defineAsyncComponent(() => import('./ServerIndex.vue'))
const SyncedIndex = defineAsyncComponent(() => import('./synced/SyncedIndex.vue'))

const route = useRoute()
const router = useRouter()
const profile = ref<GameInstance | null>(null)
const isServerProfile = computed(() => profile.value?.profile_type === 'server')
const isSyncedProfile = computed(() => profile.value?.profile_type === 'synced')

async function refreshProfile() {
	let p: GameInstance | null = null
	try {
		p = await get(route.params.id as string)
	} catch {
		p = null
	}

	if (!p) {
		// No app-lib record — treat as Core-only server instance.
		// ServerIndex will handle the actual Core lookup via route param.
		profile.value = { profile_type: 'server', path: route.params.id as string } as GameInstance
		return
	}

	profile.value = p
	if (profile.value?.profile_type === 'client') return

	if (route.name === 'Logs' || route.name === 'InstanceWorlds' || route.name === 'ModsFilter') {
		await router.replace({
			path: `/instance/${encodeURIComponent(route.params.id as string)}`,
			query: route.query,
		})
	}
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
