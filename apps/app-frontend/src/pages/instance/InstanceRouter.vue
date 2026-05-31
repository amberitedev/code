<template>
	<ServerIndex v-if="isServerProfile" />
	<ClientIndex v-else />
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { get } from '@/helpers/profile'
import type { GameInstance } from '@/helpers/types'

import ClientIndex from './Index.vue'

const ServerIndex = defineAsyncComponent(() => import('./ServerIndex.vue'))

const route = useRoute()
const router = useRouter()
const profile = ref<GameInstance | null>(null)
const isServerProfile = computed(() => profile.value?.profile_type === 'server')

async function refreshProfile() {
	profile.value = await get(route.params.id as string)
	if (profile.value?.profile_type !== 'server') return

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
</script>
