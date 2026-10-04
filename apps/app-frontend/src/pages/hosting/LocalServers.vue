<script setup lang="ts">
import { PlusIcon, SearchIcon } from '@modrinth/assets'
import { Button, ServerListing, StyledInput, injectModrinthClient } from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'

const client = injectModrinthClient()
const router = useRouter()
const queryClient = useQueryClient()
const searchInput = ref('')
const creating = ref(false)
const creationError = ref('')
const { data, error, isLoading, refetch } = useQuery({
	queryKey: ['servers'],
	queryFn: () => client.archon.servers_v0.list({ limit: 100 }),
	staleTime: 30_000,
})
const servers = computed(() =>
	(data.value?.servers ?? []).filter((server) =>
		server.name.toLowerCase().includes(searchInput.value.toLowerCase()),
	),
)

async function createServer() {
	if (creating.value) return
	creating.value = true
	creationError.value = ''
	try {
		const server = await client.archon.servers_v1.createLocal('New server')
		await queryClient.invalidateQueries({ queryKey: ['servers'] })
		await router.push(`/hosting/manage/${encodeURIComponent(server.id)}`)
	} catch (error) {
		creationError.value = error instanceof Error ? error.message : String(error)
	} finally {
		creating.value = false
	}
}
</script>

<template>
	<div data-pyro-server-list-root class="relative mx-auto flex w-full flex-col p-6">
		<div class="flex w-full flex-row items-center justify-between gap-2 mb-4">
			<h1 class="text-2xl m-0 font-extrabold text-contrast">Servers</h1>
			<div class="flex items-center gap-2">
				<StyledInput v-model="searchInput" :icon="SearchIcon" placeholder="Search servers" />
				<Button type="colored" color="brand" :disabled="creating" @click="createServer">
					<PlusIcon /> New server
				</Button>
			</div>
		</div>
		<div v-if="error" class="flex flex-col gap-3">
			<p class="text-red">{{ error.message }}</p>
			<Button @click="refetch()">Retry</Button>
		</div>
		<p v-else-if="isLoading">Loading servers...</p>
		<div v-else class="flex flex-col gap-3">
			<ServerListing v-for="server in servers" :key="server.server_id" v-bind="server" />
			<p v-if="!servers.length">No servers found.</p>
		</div>
		<p v-if="creationError" class="text-red">{{ creationError }}</p>
	</div>
</template>
