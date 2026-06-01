<script setup lang="ts">
import { computed, onMounted, watch } from 'vue'

import { useSocial } from '@/composables/useSocial'
import { useSyncedServers } from '@/composables/useSyncedServers'
import CoreServerCard from '@/pages/core/CoreServerCard.vue'

defineOptions({ name: 'CoreServersPage' })

const { group, members } = useSocial()
const { profiles, loading, error, refresh, updateSettings, getWhitelist } = useSyncedServers()

const groupId = computed(() => group.value?.group.id ?? null)

async function load() {
	if (groupId.value) await refresh(groupId.value)
}

onMounted(load)
watch(groupId, load)
</script>

<template>
	<div class="flex flex-col gap-4 w-full">
		<div v-if="!groupId" class="text-secondary">
			Join or create a Core group to manage synced servers.
		</div>
		<div v-else-if="loading && profiles.length === 0" class="text-secondary">
			Loading synced servers…
		</div>
		<p v-else-if="error" class="text-red m-0">{{ error.message }}</p>

		<template v-else>
			<p v-if="profiles.length === 0" class="text-secondary m-0">
				No synced servers yet. They appear here once an instance is synced to this group's Core.
			</p>
			<CoreServerCard
				v-for="profile in profiles"
				:key="profile._id"
				:profile="profile"
				:members="members"
				:save="updateSettings"
				:load-whitelist="getWhitelist"
			/>
		</template>
	</div>
</template>
