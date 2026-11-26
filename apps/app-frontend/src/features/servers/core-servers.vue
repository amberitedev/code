<script setup lang="ts">
import type { ListedCore } from '@amberite/contracts/backend'
import { LinkIcon, PlusIcon, SearchIcon, SettingsIcon, UpdatedIcon } from '@modrinth/assets'
import {
	Button,
	type CreationFlowContextValue,
	CreationFlowModal,
	EmptyState,
	GhostBlock,
	injectAuth,
	injectInstanceServerSource,
	injectModrinthClient,
	injectNotificationManager,
	injectServerEula,
	Input,
	installServerCreationConfig,
	ServerListing,
	useGhostReveal,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import AccountRequired from '@/features/cores/account-required.vue'
import ChooseCoreModal from '@/features/cores/choose-core-modal.vue'
import { injectCoreActions, useCores } from '@/features/cores/use-cores'
import { coreOffline, coreProblem, useServers } from '@/features/servers/linked-servers'
import { injectSettingsActions } from '@/features/settings/settings-actions'

const client = injectModrinthClient()
const auth = injectAuth()
const { addNotification } = injectNotificationManager()
const { ask: confirmEula } = injectServerEula()
const instanceSource = injectInstanceServerSource(null)
const { openCores: openCoreSettings } = injectSettingsActions()
const router = useRouter()
const queryClient = useQueryClient()
const searchInput = ref('')
const setupModal = ref<InstanceType<typeof CreationFlowModal>>()
const chooseCore = ref<InstanceType<typeof ChooseCoreModal>>()
const coresQuery = useCores()
const { openAddCore } = injectCoreActions()
// Only once the account's Cores are known, so a slow or failed lookup never asks for a Core.
const needsCore = computed(() => coresQuery.isSuccess.value && coresQuery.cores.value.length === 0)
// The Core the new server goes on, chosen before setup opens.
let core: ListedCore | null = null

async function newServer() {
	core = (await chooseCore.value?.pick()) ?? null
	if (core) setupModal.value?.show()
}
const { data, isFetching, refetch } = useServers()
// The list is checked in the background, so the answer is normally already here. Ghost rows only
// cover a cold start that has not answered within a moment.
const waitedForAnswer = ref(false)
onMounted(() => setTimeout(() => (waitedForAnswer.value = true), 300))
const serversReveal = useGhostReveal(() => !data.value && waitedForAnswer.value)
const allServers = computed(() => data.value?.servers ?? [])
const servers = computed(() =>
	allServers.value.filter((server) =>
		server.name.toLowerCase().includes(searchInput.value.toLowerCase()),
	),
)

// Never show the raw error. Either the account's Cores could not be looked up, or the Core
// itself could not be used.
const problem = computed(() => {
	const coresError = coresQuery.data.value ? null : coresQuery.error.value
	if (coresError) {
		return {
			type: 'error' as const,
			heading: 'Could not load your Cores',
			description: 'Check your connection, then try again.',
			retrying: coresQuery.isFetching.value,
			retry: () => coresQuery.refetch(),
		}
	}
	if (!coreOffline.value) return null
	return {
		type: 'no-connection' as const,
		heading: "Can't reach your Core",
		description: "Check that your Core is running and that you're online, then try again.",
		retrying: isFetching.value,
		// Offline means no socket is open, so reopen them now rather than at their next backoff.
		retry: () => {
			for (const { id } of coresQuery.cores.value) {
				void client
					.coreEvents(id)
					.ready()
					.catch(() => undefined)
			}
			return refetch()
		},
	}
})

async function searchModpacks(query: string, limit: number = 10) {
	return client.labrinth.projects_v2.search({
		query: query || undefined,
		facets: [['project_type:modpack'], ['client_side:required'], ['server_side:required']],
		limit,
	})
}

async function getProjectVersions(projectId: string) {
	const versions = await client.labrinth.versions_v3.getProjectVersions(projectId)
	return versions.map((version) => ({ id: version.id }))
}

function serverName(config: CreationFlowContextValue) {
	return (
		config.instanceSelection.value?.name ??
		config.modpackSelection.value?.name ??
		config.modpackFile.value?.name.replace(/\.mrpack$/i, '') ??
		'New server'
	)
}

/** Sets up a server that already exists, from what was chosen in the setup modal. */
async function install(coreId: string, serverId: string, config: CreationFlowContextValue) {
	const instance = config.instanceSelection.value
	if (instance && instanceSource) {
		await instanceSource.pushInstance({
			serverId,
			instanceId: instance.id,
			properties: config.buildProperties(),
		})
		return
	}
	await installServerCreationConfig(client, config, coreId, serverId)
}

/**
 * The server is only made once setup is confirmed, so cancelling leaves nothing behind. Once
 * made it stays: if its content then fails to arrive, the server's own page offers setup again,
 * and a server from an instance shows a banner on that instance.
 */
async function createServer(config: CreationFlowContextValue) {
	const instance = config.instanceSelection.value
	try {
		if (!core) throw new Error('No Core was chosen')
		const created = await client.cores.createServer(serverName(config), core, confirmEula)
		if (!created) return
		const { id } = created
		try {
			await install(core.id, id, config)
		} catch (cause) {
			console.error('Failed to set up the new server', cause)
			addNotification({
				type: 'error',
				title: 'Server created, but not set up',
				text: instance
					? 'The instance could not be sent to it. Push an update from the instance to try again.'
					: 'Its content could not be installed. Set it up again from the server.',
			})
		}
		setupModal.value?.hide()
		await queryClient.invalidateQueries({ queryKey: ['servers'] })
		await router.push(`/hosting/manage/${encodeURIComponent(id)}`)
	} catch (cause) {
		console.error('Failed to create server', cause)
		addNotification({
			type: 'error',
			title: 'Could not create the server',
			text: coreProblem(cause),
		})
	} finally {
		config.loading.value = false
	}
}

/** Browsing modpacks installs onto a server, so this one path makes the server first. */
async function browseModpacks() {
	try {
		if (!core) throw new Error('No Core was chosen')
		const created = await client.cores.createServer('New server', core, confirmEula)
		if (!created) return
		const { id } = created
		setupModal.value?.hide()
		await queryClient.invalidateQueries({ queryKey: ['servers'] })
		await router.push({ path: '/browse/modpack', query: { sid: id, from: 'onboarding' } })
	} catch (cause) {
		console.error('Failed to create server', cause)
		addNotification({
			type: 'error',
			title: 'Could not create the server',
			text: coreProblem(cause),
		})
	}
}
</script>

<template>
	<div data-pyro-server-list-root class="relative mx-auto flex h-full w-full flex-col p-6">
		<!-- Only over a list, so every empty and problem state stays centered. -->
		<div
			v-if="auth.user.value && !problem && !needsCore && allServers.length > 0"
			class="mb-4 flex w-full flex-row items-center justify-between gap-2"
		>
			<h1 class="m-0 text-2xl font-extrabold text-contrast">Servers</h1>
			<div class="flex items-center gap-2">
				<Input v-model="searchInput" :icon="SearchIcon" placeholder="Search servers" />
				<Button type="colored" color="brand" @click="newServer"><PlusIcon /> New server</Button>
			</div>
		</div>

		<div
			v-if="!auth.session_token.value"
			class="flex min-h-[60vh] grow items-center justify-center"
		>
			<AccountRequired description="Sign in to host servers on a Core you control" />
		</div>
		<div v-else-if="needsCore" class="flex min-h-[60vh] grow items-center justify-center">
			<EmptyState
				type="empty"
				heading="Add a Core to host servers"
				description="A Core runs your servers on a computer you control."
			>
				<template #actions>
					<Button type="colored" color="brand" size="lg" @click="openAddCore()">
						<LinkIcon /> Add Core
					</Button>
				</template>
			</EmptyState>
		</div>
		<div v-else-if="problem" class="flex min-h-[60vh] grow items-center justify-center">
			<EmptyState
				:type="problem.type"
				:heading="problem.heading"
				:description="problem.description"
			>
				<template #actions>
					<Button :disabled="problem.retrying" @click="problem.retry()">
						<UpdatedIcon /> Try again
					</Button>
					<Button @click="openCoreSettings"><SettingsIcon /> Core settings</Button>
				</template>
			</EmptyState>
		</div>
		<!-- Ghost rows while the list loads. Static on purpose: no shimmer. -->
		<div v-else-if="!data && waitedForAnswer" class="flex flex-col gap-3" aria-busy="true">
			<div
				v-for="row in 3"
				:key="row"
				class="flex items-center gap-4 rounded-2xl border border-solid border-surface-4 bg-bg-raised p-4"
			>
				<GhostBlock class="size-16 shrink-0 !rounded-xl" />
				<div class="flex flex-col gap-2.5">
					<GhostBlock class="h-5 w-48" />
					<GhostBlock class="h-4 w-72" />
				</div>
			</div>
		</div>
		<div
			v-else-if="data && allServers.length === 0"
			class="flex min-h-[60vh] grow items-center justify-center"
			:class="serversReveal"
		>
			<EmptyState
				type="empty"
				heading="No servers yet"
				description="Create a server on your Core and play with your friends."
			>
				<template #actions>
					<Button type="colored" color="brand" size="lg" @click="newServer">
						<PlusIcon /> New server
					</Button>
				</template>
			</EmptyState>
		</div>
		<div v-else-if="data" class="flex flex-col gap-3" :class="serversReveal">
			<ServerListing
				v-for="server in servers"
				:key="server.server_id"
				v-bind="server"
				game="Minecraft"
			/>
			<EmptyState
				v-if="servers.length === 0"
				type="no-search-result"
				heading="No servers match your search"
			/>
		</div>

		<ChooseCoreModal ref="chooseCore" />
		<CreationFlowModal
			ref="setupModal"
			type="server-onboarding"
			:available-loaders="['vanilla', 'fabric', 'neoforge', 'forge', 'quilt', 'paper']"
			:show-snapshot-toggle="true"
			:search-projects="searchModpacks"
			:get-project-versions="getProjectVersions"
			@browse-modpacks="browseModpacks"
			@create="createServer"
		/>
	</div>
</template>
