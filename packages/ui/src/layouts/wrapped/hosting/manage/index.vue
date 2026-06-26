<template>
	<div
		data-core-server-list-root
		class="relative mx-auto mb-6 flex w-full flex-col gap-4 p-6"
	>
		<div class="flex flex-col justify-between gap-3 md:flex-row md:items-center">
			<div class="flex flex-col gap-1">
				<h1 class="m-0 text-3xl font-bold text-contrast">Amberite Hosting</h1>
				<p class="m-0 text-secondary">Core-managed servers available on this Amberite Core.</p>
			</div>
			<ButtonStyled color="brand">
				<button @click="createModal?.show()">
					<PlusIcon />
					Create server
				</button>
			</ButtonStyled>
		</div>

		<div v-if="serversQuery.error.value" class="rounded-2xl bg-red-highlight p-4 text-contrast">
			Failed to load servers: {{ (serversQuery.error.value as Error).message }}
		</div>

		<div v-if="serversQuery.isLoading.value" class="rounded-2xl bg-surface-3 p-6 text-secondary">
			Loading servers...
		</div>

		<div v-else-if="servers.length === 0" class="rounded-2xl bg-surface-3 p-8 text-secondary">
			No Core servers have been created yet.
		</div>

		<div v-else class="flex flex-col gap-3">
			<RouterLink
				v-for="server in servers"
				:key="server.id"
				:to="`${manageBasePath}/${encodeURIComponent(server.id)}`"
				class="flex items-center gap-4 rounded-2xl border border-solid border-surface-4 bg-surface-2 p-4 text-primary no-underline transition-transform hover:brightness-125 active:scale-[0.99]"
			>
				<ServerIcon class="size-14 !rounded-xl" />
				<div class="flex min-w-0 flex-1 flex-col gap-1">
					<div class="flex min-w-0 items-center gap-2">
						<h2 class="m-0 truncate text-xl font-bold text-contrast">{{ server.name }}</h2>
						<span class="rounded-full bg-surface-4 px-2 py-0.5 text-xs font-semibold capitalize">
							{{ server.status }}
						</span>
					</div>
					<div class="flex flex-wrap gap-2 text-sm text-secondary">
						<span>{{ formatLoader(server.loader) }} {{ server.game_version }}</span>
						<span>127.0.0.1:{{ server.port }}</span>
						<span>{{ server.memory.max_mb }} MB</span>
					</div>
				</div>
			</RouterLink>
		</div>

		<CoreServerCreateModal ref="createModal" @created="openCreatedServer" />
	</div>
</template>

<script setup lang="ts">
import { PlusIcon } from '@modrinth/assets'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref } from 'vue'
import { RouterLink, useRouter } from 'vue-router'

import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import ServerIcon from '#ui/components/servers/icons/ServerIcon.vue'
import { injectHostingBackend } from '#ui/providers'

import CoreServerCreateModal from './CoreServerCreateModal.vue'

const props = withDefaults(
	defineProps<{
		manageBasePath?: string
	}>(),
	{
		manageBasePath: '/hosting/manage',
	},
)

const backend = injectHostingBackend()
const router = useRouter()
const queryClient = useQueryClient()
const createModal = ref<InstanceType<typeof CoreServerCreateModal>>()
const serversQuery = useQuery({
	queryKey: ['core-hosting', 'servers'],
	queryFn: () => backend.listServers(),
	staleTime: 15_000,
})
const servers = computed(() => serversQuery.data.value ?? [])

function formatLoader(loader: string) {
	if (loader === 'neoforge') return 'NeoForge'
	return loader.charAt(0).toUpperCase() + loader.slice(1)
}

async function openCreatedServer(id: string) {
	await queryClient.invalidateQueries({ queryKey: ['core-hosting', 'servers'] })
	await router.push(`${props.manageBasePath}/${encodeURIComponent(id)}`)
}
</script>
