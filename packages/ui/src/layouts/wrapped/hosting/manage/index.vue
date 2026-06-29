<template>
	<div
		data-core-server-list-root
		class="relative mx-auto mb-6 flex w-full flex-col p-6"
		:class="servers.length ? 'min-h-screen' : 'min-h-[calc(100vh-14.5rem)]'"
	>
		<CoreServerCreateModal ref="createModal" @created="openCreatedServer" />

		<div v-if="hasError" class="rounded-2xl bg-red-highlight p-4 text-contrast">
			Failed to load servers: {{ (serversQuery.error.value as Error).message }}
		</div>

		<template v-else>
			<div
				v-if="!showEmptyState"
				class="relative mb-4 flex h-fit w-full flex-col items-center justify-between md:flex-row"
			>
				<h1 class="m-0 w-full text-2xl font-extrabold text-contrast">
					{{ formatMessage(messages.serversTitle) }}
				</h1>
				<div class="flex w-full flex-row items-center justify-end gap-2 md:mb-0">
					<StyledInput
						id="search"
						v-model="searchInput"
						:icon="SearchIcon"
						type="search"
						name="search"
						autocomplete="off"
						:disabled="showServersListLoading"
						:placeholder="formatMessage(messages.searchPlaceholder, { count: filteredServers.length })"
						wrapper-class="w-full md:w-72"
					/>
					<ButtonStyled type="standard" color="brand">
						<button @click="createModal?.show()">
							<PlusIcon />
							{{ formatMessage(messages.newServerButton) }}
						</button>
					</ButtonStyled>
				</div>
			</div>

			<Transition name="fade" mode="out-in">
				<div v-if="showServersListLoading" key="loading" class="flex flex-col gap-3">
					<div
						v-for="i in 3"
						:key="i"
						class="flex animate-pulse flex-row items-center gap-4 overflow-x-hidden rounded-2xl border-[1px] border-solid border-button-bg bg-bg-raised p-4"
					>
						<div class="size-16 rounded-xl bg-button-bg"></div>
						<div class="flex flex-1 flex-col gap-2">
							<div class="h-6 w-48 rounded bg-button-bg"></div>
							<div class="h-4 w-64 rounded bg-button-bg opacity-75"></div>
						</div>
					</div>
				</div>

				<div
					v-else-if="showEmptyState"
					key="empty"
					class="flex h-full max-h-[1100px] grow flex-col items-center justify-center gap-8"
				>
					<ServerListEmpty :logged-in="true" @click-new-server="createModal?.show()" />
				</div>

				<div v-else key="list" class="flex flex-col gap-6">
					<section class="flex flex-col gap-3">
						<h2 class="m-0 text-xl font-semibold text-primary">
							{{ formatMessage(messages.yourServersTitle) }}
						</h2>
						<TransitionGroup
							v-if="filteredServers.length > 0"
							name="list"
							tag="ul"
							class="m-0 flex flex-col gap-3 p-0"
						>
							<li v-for="server in filteredServers" :key="server.id" class="list-none">
								<RouterLink
									:to="`${manageBasePath}/${encodeURIComponent(server.path)}`"
									class="block cursor-pointer text-primary no-underline transition-all"
								>
									<div
										class="flex flex-row items-center overflow-x-hidden rounded-2xl border-[1px] border-solid border-surface-4 bg-bg-raised p-4 transition-all duration-150 hover:brightness-125 active:scale-[0.985]"
									>
										<ServerIcon class="!rounded-xl" />
										<div class="ml-4 flex min-w-0 flex-1 flex-col gap-1.5">
											<div class="flex min-w-0 flex-row items-center gap-2.5">
												<h3 class="m-0 truncate text-xl font-bold text-contrast">
													{{ server.name }}
												</h3>
												<div
													class="flex min-w-0 items-center gap-2 rounded-full border border-solid px-2.5 h-[28px] text-sm font-medium"
													:class="statusTone(server).badge"
												>
													{{ statusLabel(server) }}
												</div>
											</div>
											<div
												class="flex w-full flex-row flex-wrap items-center gap-2 text-primary"
											>
												<span class="rounded-full bg-surface-4 px-2.5 py-1 text-sm font-medium">
													{{ formatLoader(server.loader) }}
												</span>
												<span class="rounded-full bg-surface-4 px-2.5 py-1 text-sm font-medium">
													{{ server.game_version }}
												</span>
												<span class="rounded-full bg-surface-4 px-2.5 py-1 text-sm font-medium">
													{{ server.portLabel }}
												</span>
												<span class="rounded-full bg-surface-4 px-2.5 py-1 text-sm font-medium">
													{{ server.memoryLabel }}
												</span>
											</div>
										</div>
									</div>
								</RouterLink>
							</li>
						</TransitionGroup>
						<div v-else class="text-secondary">
							{{ formatMessage(messages.noServersFound) }}
						</div>
					</section>
				</div>
			</Transition>
		</template>
	</div>
</template>

<script setup lang="ts">
import type { CoreInstanceSummary } from '@amberite/amberite-api'
import { PlusIcon, SearchIcon } from '@modrinth/assets'
import {
	ButtonStyled,
	defineMessages,
	ServerListEmpty,
	StyledInput,
	useVIntl,
} from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import Fuse from 'fuse.js'
import { computed, ref } from 'vue'
import { RouterLink, useRouter } from 'vue-router'

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

const { formatMessage } = useVIntl()

const messages = defineMessages({
	serversTitle: { id: 'servers.manage.servers-title', defaultMessage: 'Modrinth Hosting' },
	searchPlaceholder: {
		id: 'servers.manage.search-placeholder',
		defaultMessage: 'Search {count} {count, plural, one {server} other {servers}}...',
	},
	newServerButton: { id: 'servers.manage.new-server-button', defaultMessage: 'New server' },
	yourServersTitle: {
		id: 'servers.manage.your-servers-title',
		defaultMessage: 'Your servers',
	},
	noServersFound: { id: 'servers.manage.no-servers-found', defaultMessage: 'No servers found.' },
})

type ServerListItem = CoreInstanceSummary & {
	portLabel: string
	memoryLabel: string
}

const backend = injectHostingBackend()
const router = useRouter()
const queryClient = useQueryClient()
const createModal = ref<InstanceType<typeof CoreServerCreateModal>>()
const searchInput = ref('')

const serversQuery = useQuery({
	queryKey: ['core-hosting', 'servers'],
	queryFn: () => backend.listServers(),
	staleTime: 15_000,
})

const hasError = computed(() => !!serversQuery.error.value)
const showServersListLoading = computed(
	() => serversQuery.isLoading.value || serversQuery.isPending.value,
)

const servers = computed<ServerListItem[]>(() =>
	(serversQuery.data.value ?? []).map((server) => ({
		...server,
		portLabel: `127.0.0.1:${server.port}`,
		memoryLabel: `${formatMemory(server.memory.max_mb)} RAM`,
	})),
)

const showEmptyState = computed(() => !showServersListLoading.value && servers.value.length === 0)

const filteredServers = computed<ServerListItem[]>(() => {
	const normalizedSearch = searchInput.value.trim()
	if (!normalizedSearch) return sortServers(servers.value)

	const fuse = new Fuse(servers.value, {
		keys: ['name', 'loader', 'game_version', 'status', 'install_status', 'portLabel', 'memoryLabel'],
		includeScore: true,
		threshold: 0.35,
	})

	return sortServers(fuse.search(normalizedSearch).map((result) => result.item))
})

function sortServers(serverList: ServerListItem[]) {
	return serverList.slice().sort((a, b) => {
		const statusDiff = statusPriority(a) - statusPriority(b)
		if (statusDiff !== 0) return statusDiff
		return a.name.localeCompare(b.name)
	})
}

function statusPriority(server: CoreInstanceSummary) {
	if (server.install_status !== 'ready') return 0
	if (server.status === 'running') return 1
	if (server.status === 'starting') return 2
	if (server.status === 'offline') return 3
	if (server.status === 'stopping') return 4
	return 5
}

function formatLoader(loader: string) {
	if (loader === 'neoforge') return 'NeoForge'
	return loader.charAt(0).toUpperCase() + loader.slice(1)
}

function formatMemory(memoryMb: number) {
	if (memoryMb % 1024 === 0) return `${memoryMb / 1024} GB`
	return `${memoryMb} MB`
}

function statusLabel(server: CoreInstanceSummary) {
	if (server.install_status === 'installing') return 'Setting up'
	if (server.install_status === 'failed') return 'Needs attention'

	switch (server.status) {
		case 'running':
			return 'Online'
		case 'starting':
			return 'Starting'
		case 'stopping':
			return 'Stopping'
		case 'crashed':
			return 'Crashed'
		default:
			return 'Offline'
	}
}

function statusTone(server: CoreInstanceSummary) {
	if (server.install_status === 'failed') {
		return { badge: 'border-red bg-red-highlight text-red' }
	}
	if (server.install_status === 'installing' || server.status === 'starting') {
		return { badge: 'border-brand bg-brand-highlight text-brand' }
	}
	if (server.status === 'running') {
		return { badge: 'border-green bg-green-highlight text-green' }
	}
	return { badge: 'border-surface-5 bg-surface-4 text-primary' }
}

async function openCreatedServer(path: string) {
	await queryClient.invalidateQueries({ queryKey: ['core-hosting', 'servers'] })
	await router.push(`${props.manageBasePath}/${encodeURIComponent(path)}`)
}
</script>

<style scoped>
.fade-enter-active,
.fade-leave-active {
	transition:
		opacity 300ms ease-in-out,
		transform 300ms ease-in-out;
}

.fade-enter-from,
.fade-leave-to {
	opacity: 0;
	transform: scale(0.98);
}

.list-enter-active,
.list-leave-active {
	transition: all 200ms ease-in-out;
}

.list-enter-from {
	opacity: 0;
	transform: translateY(-10px);
}

.list-leave-to {
	opacity: 0;
	transform: translateY(10px);
}

.list-move {
	transition: transform 200ms ease-in-out;
}
</style>
