<template>
	<AddServerModal
		ref="addServerModal"
		:instance="instance"
		@submit="
			(server, start) => {
				addServer(server)
				if (start) {
					joinWorld(server)
				}
			}
		"
	/>
	<EditServerModal ref="editServerModal" :instance="instance" @submit="editServer" />
	<EditWorldModal ref="editWorldModal" :instance="instance" @submit="editWorld" />
	<ConfirmRemoveWorldModal
		ref="removeWorldModal"
		:world="worldToRemove"
		@confirm="proceedRemoveWorld"
	/>
	<DesyncServerModal ref="desyncServerModal" @confirm="confirmDesyncServer" />
	<UpdateBeforeJoinModal ref="updateBeforeJoinModal" />
	<CreateServerFromInstanceModal
		v-if="canCreateServer"
		ref="createServerModal"
		:instance="instance"
	/>
	<!-- Ghost rows while the first answer is pending. Static on purpose: no shimmer. -->
	<div v-if="worldsReadyPending" class="flex flex-col gap-2" aria-busy="true">
		<GhostBlock class="h-10 w-full !rounded-xl" />
		<GhostBlock class="h-9 w-24 self-end" />
		<div class="mt-2 flex flex-col gap-2">
			<div
				v-for="row in 3"
				:key="row"
				class="flex min-h-20 items-center gap-2 rounded-[20px] border border-solid border-surface-4 bg-bg-raised p-3"
			>
				<GhostBlock class="size-12 shrink-0 !rounded-[14px]" />
				<div class="flex flex-col gap-2">
					<GhostBlock class="h-4 w-40" />
					<GhostBlock class="h-3.5 w-56" />
				</div>
			</div>
		</div>
	</div>
	<template v-else>
		<div v-if="rows.length > 0" class="flex flex-col gap-2" :class="worldsReveal">
			<div class="flex flex-wrap items-center gap-2">
				<Input
					v-model="searchFilter"
					:icon="SearchIcon"
					type="text"
					autocomplete="off"
					:spellcheck="false"
					size="medium"
					wrapper-class="flex-1 min-w-0"
					clearable
					:placeholder="formatMessage(messages.searchWorldsPlaceholder, { count: rows.length })"
				/>
				<div class="flex gap-2">
					<TeleportOverflowMenu
						v-if="canCreateServer"
						type="outlined"
						size="lg"
						:icon-only="false"
						:circular="false"
						:label="formatMessage(messages.addServer)"
						:options="addServerOptions"
					>
						<PlusIcon class="size-5" />
						{{ formatMessage(messages.addServer) }}
						<DropdownIcon class="size-5" />
					</TeleportOverflowMenu>
					<Button v-else type="outlined" size="lg" @click="addServerModal?.show()">
						<PlusIcon class="size-5" />
						{{ formatMessage(messages.addServer) }}
					</Button>
					<Button type="colored" color="brand" size="lg" @click="instancePage.browseServers">
						<CompassIcon class="size-5" />
						<span>{{ formatMessage(messages.browseServers) }}</span>
					</Button>
				</div>
			</div>
			<div class="flex flex-wrap items-center justify-between gap-2">
				<FilterPills
					:model-value="selectedFilters"
					:options="filterOptions"
					@update:model-value="updateFilters"
				>
					<template #all>
						{{ formatMessage(commonMessages.allProjectType) }}
					</template>
				</FilterPills>
				<Button
					type="quiet"
					:disabled="refreshingAll"
					class="!text-sm !font-medium"
					@click="refreshAllWorlds"
				>
					<RefreshCwIcon :class="refreshingAll ? 'animate-spin' : ''" />
					{{
						formatMessage(refreshingAll ? messages.refreshingButton : commonMessages.refreshButton)
					}}
				</Button>
			</div>
			<!-- One list: pinned rows first, in the order they were dragged into. -->
			<Draggable
				:model-value="filteredRows"
				item-key="key"
				class="mt-2 flex w-full flex-col gap-2"
				handle=".world-drag-handle"
				:disabled="filtering"
				:animation="150"
				:move="staysAmongPinned"
				@update:model-value="reorderPinned"
			>
				<template #item="{ element: row }: { element: WorldRow }">
					<WorldItem
						:world="row.world"
						:managed="isManagedServerWorld(row.world)"
						:linked-server-id="row.linked?.server_id"
						:linked-note="row.linked && linkedNote(row.linked)"
						:copy-address="row.linked && shareableServerAddress(row.linked)"
						:outside-warning="row.linked && unreachableCause(row.linked)"
						:hosted-by="row.hosted?.host.username"
						:hosted-server-id="
							row.hosted && canManageHostedServer(row.hosted) ? row.hosted.server_id : undefined
						"
						pinnable
						:pinned="pins.pinned.value.includes(row.key)"
						:highlighted="highlightedWorld === getWorldIdentifier(row.world)"
						:supports-server-quick-play="supportsServerQuickPlay"
						:supports-world-quick-play="supportsWorldQuickPlay"
						:quarantined="instance.quarantined"
						:current-protocol="protocolVersion"
						:playing-instance="playing"
						:playing-world="worldsMatch(row.world, worldPlaying)"
						:starting-instance="startingInstance"
						:refreshing="
							row.world.type === 'server' ? serverData[row.world.address]?.refreshing : undefined
						"
						:server-status="
							row.world.type === 'server' ? serverData[row.world.address]?.status : undefined
						"
						:rendered-motd="
							row.world.type === 'server' ? serverData[row.world.address]?.renderedMotd : undefined
						"
						:game-mode="
							row.world.type === 'singleplayer' ? GAME_MODES[row.world.game_mode] : undefined
						"
						:shortcut-instance-id="instance.id"
						@play="() => joinRow(row)"
						@stop="() => instancePage.stop('InstanceWorlds')"
						@refresh="() => row.world.type === 'server' && refreshServer(row.world.address)"
						@toggle-pin="() => pins.toggle(row.key)"
						@edit="() => editRow(row)"
						@delete="
							() =>
								ownRow(row) && !isManagedServerWorld(row.world) && promptToRemoveWorld(row.world)
						"
						@desync="() => row.world.type === 'server' && desyncServerModal?.show(row.world)"
						@open-folder="(world: SingleplayerWorld) => showWorldInFolder(instance.id, world.path)"
					/>
				</template>
			</Draggable>
		</div>
		<EmptyState
			v-else
			type="empty-inbox"
			:class="worldsReveal"
			:heading="formatMessage(messages.noWorldsHeading)"
			:description="formatMessage(messages.noWorldsDescription)"
		>
			<template #actions>
				<TeleportOverflowMenu
					v-if="canCreateServer"
					type="outlined"
					size="lg"
					:icon-only="false"
					:circular="false"
					:label="formatMessage(messages.addServer)"
					:options="addServerOptions"
				>
					<PlusIcon class="size-5" />
					{{ formatMessage(messages.addServer) }}
					<DropdownIcon class="size-5" />
				</TeleportOverflowMenu>
				<Button v-else type="outlined" size="lg" @click="addServerModal?.show()">
					<PlusIcon class="size-5" />
					{{ formatMessage(messages.addServer) }}
				</Button>
				<Button type="colored" color="brand" size="lg" @click="instancePage.browseServers">
					<CompassIcon class="size-5" />
					<span>{{ formatMessage(messages.browseServers) }}</span>
				</Button>
			</template>
		</EmptyState>
	</template>
</template>
<script setup lang="ts">
import {
	CompassIcon,
	DropdownIcon,
	GlobeIcon,
	PlusIcon,
	RefreshCwIcon,
	SearchIcon,
	ServerPlusIcon,
} from '@modrinth/assets'
import { Button, type ButtonMenuOption, TeleportOverflowMenu } from '@modrinth/ui'
import {
	commonMessages,
	defineMessages,
	EmptyState,
	FilterPills,
	GAME_MODES,
	type GameVersion,
	GhostBlock,
	injectNotificationManager,
	Input,
	useGhostReveal,
	useLoadingBarToken,
	useReadyState,
	useVIntl,
} from '@modrinth/ui'
import { queryOptions, useQuery, useQueryClient } from '@tanstack/vue-query'
import { platform } from '@tauri-apps/plugin-os'
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import Draggable from 'vuedraggable'

import CreateServerFromInstanceModal from '@/components/ui/linked-servers/CreateServerFromInstanceModal.vue'
import AddServerModal from '@/components/ui/world/modal/AddServerModal.vue'
import ConfirmRemoveWorldModal from '@/components/ui/world/modal/ConfirmRemoveWorldModal.vue'
import DesyncServerModal from '@/components/ui/world/modal/DesyncServerModal.vue'
import EditServerModal from '@/components/ui/world/modal/EditServerModal.vue'
import EditWorldModal from '@/components/ui/world/modal/EditSingleplayerWorldModal.vue'
import UpdateBeforeJoinModal from '@/components/ui/world/modal/UpdateBeforeJoinModal.vue'
import WorldItem from '@/components/ui/world/WorldItem.vue'
import { useAppEvent } from '@/composables/use-app-event'
import { handleSevereError } from '@/composables/use-error.js'
import { trackEvent } from '@/helpers/analytics'
import { get_project, get_project_v3 } from '@/helpers/cache.js'
import { set_synced_option } from '@/helpers/instance'
import {
	coreOffline,
	type HostedServer,
	type LinkedServer,
	linkedServerState,
	serverAddress,
	shareableServerAddress,
	unreachableCause,
	useCanManageHostedServer,
	useHostedServers,
	useInstanceServer,
} from '@/helpers/linked-servers'
import { get_game_versions } from '@/helpers/tags'
import { useWorldPins, worldPinKey } from '@/helpers/world-pins'
import {
	delete_world,
	desync_server,
	type DesyncServerMode,
	ensureManagedServerWorldExists,
	get_instance_protocol_version,
	getServerAddress,
	getWorldIdentifier,
	handleDefaultInstanceUpdateEvent,
	hasServerQuickPlaySupport,
	hasWorldQuickPlaySupport,
	normalizeServerAddress,
	type ProtocolVersion,
	refreshServerData,
	refreshServers,
	refreshWorld,
	remove_server_from_instance,
	resolveManagedServerWorld,
	type ServerData,
	type ServerWorld,
	showWorldInFolder,
	type SingleplayerWorld,
	sortWorlds,
	start_join_server,
	start_join_singleplayer_world,
	type World,
	worldNameMatchesQuery,
} from '@/helpers/worlds.ts'
import { injectServerInstall } from '@/providers/server-install'

import { injectInstancePage } from '../instance-context'
import { instanceKeys, instanceWorldsQueryOptions } from '../query-options'
import { injectSharedInstance } from '../shared-instance-context'

const messages = defineMessages({
	searchWorldsPlaceholder: {
		id: 'app.instance.worlds.search-worlds-placeholder',
		defaultMessage: 'Search {count} worlds...',
	},
	addServer: {
		id: 'app.instance.worlds.add-server',
		defaultMessage: 'Add server',
	},
	addByAddress: {
		id: 'app.instance.worlds.add-by-address',
		defaultMessage: 'Add by address',
	},
	createServer: {
		id: 'app.instance.worlds.create-server',
		defaultMessage: 'Create server',
	},
	linkedUpdating: {
		id: 'app.instance.worlds.linked.updating',
		defaultMessage: 'Updating from this instance',
	},
	linkedPending: {
		id: 'app.instance.worlds.linked.pending',
		defaultMessage: 'Update applies when the server stops',
	},
	linkedFailed: {
		id: 'app.instance.worlds.linked.failed',
		defaultMessage: 'Update failed',
	},
	linkedCoreOffline: {
		id: 'app.instance.worlds.linked.core-offline',
		defaultMessage: 'Core offline',
	},
	browseServers: {
		id: 'app.instance.worlds.browse-servers',
		defaultMessage: 'Browse servers',
	},
	noWorldsHeading: {
		id: 'app.instance.worlds.no-worlds-heading',
		defaultMessage: 'No servers or worlds added',
	},
	noWorldsDescription: {
		id: 'app.instance.worlds.no-worlds-description',
		defaultMessage: 'Add a server or browse to get started',
	},
	vanillaFilter: {
		id: 'app.instance.worlds.filter-vanilla',
		defaultMessage: 'Vanilla',
	},
	moddedFilter: {
		id: 'app.instance.worlds.filter-modded',
		defaultMessage: 'Modded',
	},
	onlineFilter: {
		id: 'app.instance.worlds.filter-online',
		defaultMessage: 'Online',
	},
	offlineFilter: {
		id: 'app.instance.worlds.filter-offline',
		defaultMessage: 'Offline',
	},
	refreshingButton: {
		id: 'app.instance.worlds.refreshing',
		defaultMessage: 'Refreshing...',
	},
})

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { playServerProject } = injectServerInstall()
const route = useRoute()
const instancePage = injectInstancePage()
const sharedInstance = injectSharedInstance()

const addServerModal = ref<InstanceType<typeof AddServerModal>>()
const editServerModal = ref<InstanceType<typeof EditServerModal>>()
const editWorldModal = ref<InstanceType<typeof EditWorldModal>>()
const removeWorldModal = ref<InstanceType<typeof ConfirmRemoveWorldModal>>()
const desyncServerModal = ref<InstanceType<typeof DesyncServerModal>>()
const createServerModal = ref<InstanceType<typeof CreateServerFromInstanceModal>>()
const updateBeforeJoinModal = ref<InstanceType<typeof UpdateBeforeJoinModal>>()

const worldToRemove = ref<World | null>(null)

const instance = computed(() => instancePage.instance.value!)
const playing = instancePage.playing

function play() {
	if (instance.value.quarantined) return
	void instancePage.refreshPlayState()
}

const selectedFilters = ref<string[]>([])
const searchFilter = ref('')

const { server: linkedServer } = useInstanceServer(() => instance.value.id)
// Offered until the instance has its one server: creating it asks for a Core first when the
// account has none, and says so when the Core cannot be reached.
const canCreateServer = computed(
	() =>
		!instance.value.quarantined &&
		instance.value.install_stage === 'installed' &&
		!linkedServer.value,
)
// Servers other people host for this shared instance. A signed-out or wrong account gets none,
// and neither does an instance that was deleted or that this account lost access to.
const { servers: hostedServers, query: hostedServersQuery } = useHostedServers(() =>
	sharedInstance.shareActionsLocked.value || sharedInstance.signedOut.value
		? undefined
		: instance.value.shared_instance?.id,
)
// The ones among them this account may manage, which makes those rows open the server page.
const canManageHostedServer = useCanManageHostedServer()

const pins = useWorldPins(() => instance.value.id)

/**
 * One row of the list. A server on the user's Core (`linked`) and a server someone else hosts
 * (`hosted`) are listed as ordinary server rows, so they look and join like the others.
 */
type WorldRow = { key: string; world: World; linked?: LinkedServer; hosted?: HostedServer }

function serverRow(key: string, name: string, address: string): ServerWorld {
	return {
		type: 'server',
		index: -1,
		name,
		display_status: 'normal',
		address,
		pack_status: 'enabled',
		// These servers are not in the instance's own list, which is where play times come from.
		last_played: pins.lastJoined(key),
	}
}

function linkedRow(linked: LinkedServer): WorldRow {
	const key = `linked:${linked.server_id}`
	return { key, linked, world: serverRow(key, linked.name, serverAddress(linked)) }
}

/** Every row: pinned ones in their dragged order, then the rest by most recently played. */
const rows = computed<WorldRow[]>(() => {
	const all: WorldRow[] = [
		...(linkedServer.value ? [linkedRow(linkedServer.value)] : []),
		...hostedServers.value.map((hosted) => {
			const key = `hosted:${hosted.core_id}:${hosted.server_id}`
			return { key, hosted, world: serverRow(key, hosted.name, hosted.address) }
		}),
		...dedupedWorlds.value.map((world) => ({ key: worldPinKey(world), world })),
	]
	const order = pins.pinned.value
	const played = (row: WorldRow) => (row.world.last_played ? Date.parse(row.world.last_played) : 0)
	return [
		...all
			.filter((row) => order.includes(row.key))
			.sort((a, b) => order.indexOf(a.key) - order.indexOf(b.key)),
		...all.filter((row) => !order.includes(row.key)).sort((a, b) => played(b) - played(a)),
	]
})

/** A row from the instance's own list, which the user may edit and remove. */
function ownRow(row: WorldRow) {
	return !row.linked && !row.hosted
}

function editRow(row: WorldRow) {
	if (!ownRow(row) || isManagedServerWorld(row.world)) return
	if (row.world.type === 'singleplayer') editWorldModal.value?.show(row.world)
	else editServerModal.value?.show(row.world)
}

// Dragging reorders the pinned rows only. It is off while a search or filter hides some of them.
const filtering = computed(() => !!searchFilter.value || selectedFilters.value.length > 0)

function staysAmongPinned(event: { relatedContext: { element?: WorldRow } }) {
	const over = event.relatedContext.element
	return !!over && pins.pinned.value.includes(over.key)
}

function reorderPinned(reordered: WorldRow[]) {
	const pinned = new Set(pins.pinned.value)
	pins.pinned.value = reordered.filter((row) => pinned.has(row.key)).map((row) => row.key)
}

/**
 * Joins a row's world. A member whose shared instance is behind is first asked whether to update
 * when the server is someone else's, since that server follows the instance.
 */
async function joinRow(row: WorldRow) {
	if (row.linked || row.hosted) pins.markJoined(row.key)
	const hosted = row.hosted
	if (hosted && instance.value.shared_instance?.role === 'member') {
		// Busy from the first click, so a second one does not check and join again.
		if (startingInstance.value) return
		startingInstance.value = true
		const preview = await sharedInstance
			.refreshUpdatePreview()
			.catch(() => sharedInstance.updatePreview.value)
			.finally(() => (startingInstance.value = false))
		if (preview?.updateAvailable && updateBeforeJoinModal.value) {
			updateBeforeJoinModal.value.show(instance.value.id, hosted.host.username, async () => {
				await instancePage.refreshInstance()
				await joinWorld(row.world)
			})
			return
		}
	}
	await joinWorld(row.world)
}

function linkedNote(server: LinkedServer) {
	// The rows are the last answer the Core gave, so its state is not known right now.
	if (coreOffline.value) {
		return { text: formatMessage(messages.linkedCoreOffline), tone: 'orange' as const }
	}
	switch (linkedServerState(server)) {
		case 'updating':
			return { text: formatMessage(messages.linkedUpdating), tone: 'blue' as const }
		case 'pending':
			return { text: formatMessage(messages.linkedPending), tone: 'orange' as const }
		case 'failed':
			return {
				text: server.linked_instance.error ?? formatMessage(messages.linkedFailed),
				tone: 'red' as const,
			}
		default:
			return undefined
	}
}

const addServerOptions = computed<ButtonMenuOption[]>(() => [
	{
		id: 'add-by-address',
		label: formatMessage(messages.addByAddress),
		icon: GlobeIcon,
		action: () => addServerModal.value?.show(),
	},
	{
		id: 'create-server',
		label: formatMessage(messages.createServer),
		icon: ServerPlusIcon,
		action: () => void createServerModal.value?.show(),
	},
])

function updateFilters(filters: string[]) {
	const addedFilter = filters.find((id) => !selectedFilters.value.includes(id))
	selectedFilters.value = filters
	if (addedFilter === 'singleplayer') {
		selectedFilters.value = filters.filter((id) => id !== 'online' && id !== 'offline')
	} else if (addedFilter === 'online' || addedFilter === 'offline') {
		selectedFilters.value = filters.filter((id) => id !== 'singleplayer')
	}
}

const queryClient = useQueryClient()

const refreshingAll = ref(false)
const hadNoWorlds = ref(true)
const startingInstance = ref(false)
const worldPlaying = ref<World>()

const worldsQuery = useQuery(
	computed(() =>
		queryOptions({
			...instanceWorldsQueryOptions(instancePage.instanceId.value),
			enabled: !!instancePage.instanceId.value,
		}),
	),
)

const worldsReadyPending = useReadyState(worldsQuery)
useLoadingBarToken(worldsReadyPending)
const worldsReveal = useGhostReveal(() => worldsReadyPending.value)

const worlds = ref<World[]>([])
const serverData = ref<Record<string, ServerData>>({})

// Track servers_updated calls on Linux to prevent server ping spam
const MAX_LINUX_REFRESHES = 3
const isLinux = platform() === 'linux'
const linuxRefreshCount = ref(0)

const protocolVersion = ref<ProtocolVersion | null>(null)
const protocolVersionReady = ref(false)
const gameVersions = ref<GameVersion[]>([])
const supportsServerQuickPlay = computed(() =>
	hasServerQuickPlaySupport(gameVersions.value, instance.value.game_version),
)
const supportsWorldQuickPlay = computed(() =>
	hasWorldQuickPlaySupport(gameVersions.value, instance.value.game_version),
)

watch(
	() => worldsQuery.data.value,
	(data) => {
		if (data) {
			worlds.value = [...data]
			hadNoWorlds.value = worlds.value.length === 0
			// Manual refresh handles its own server pings to avoid double-pinging
			if (!refreshingAll.value) {
				void refreshServers(
					worlds.value,
					serverData.value,
					protocolVersion.value,
					protocolVersionReady.value,
				)
			}
		}
	},
	{ immediate: true },
)
const managedServerName = ref<string | null>(null)
const managedServerAddress = ref<string | null>(null)

const managedServerWorld = computed(() =>
	resolveManagedServerWorld(worlds.value, managedServerName.value, managedServerAddress.value),
)

function isManagedServerWorld(world: World): world is ServerWorld {
	return world.type === 'server' && managedServerWorld.value?.index === world.index
}

async function refreshManagedServerMetadata() {
	const instanceId = instance.value.id
	const projectId = instance.value.link?.project_id
	if (!projectId) {
		managedServerName.value = null
		managedServerAddress.value = null
		return
	}

	try {
		const [project, projectV3] = await Promise.all([
			get_project(projectId),
			get_project_v3(projectId),
		])
		if (instance.value.id !== instanceId || instance.value.link?.project_id !== projectId) return

		if (projectV3?.minecraft_server == null) {
			managedServerName.value = null
			managedServerAddress.value = null
			return
		}

		const serverAddress = getServerAddress(projectV3.minecraft_java_server)
		if (!serverAddress) {
			managedServerName.value = null
			managedServerAddress.value = null
			return
		}

		managedServerName.value = project.title
		managedServerAddress.value = serverAddress
		await ensureManagedServerWorldExists(instanceId, project.title, serverAddress)
		await queryClient.invalidateQueries({ queryKey: instanceKeys.worlds(instanceId) })
	} catch (err) {
		console.error(
			`Failed to resolve managed server metadata for instance: ${instance.value.id}`,
			err,
		)
		managedServerName.value = null
		managedServerAddress.value = null
	}
}

watch(
	() => [instance.value.id, instance.value.link?.project_id],
	async () => {
		await refreshManagedServerMetadata()
	},
	{ immediate: true },
)

let worldsTabAlive = true

useAppEvent('instance', async (event) => {
	if (event.instance_id !== instance.value.id) return

	console.info(`Handling instance event '${event.event}' for instance: ${event.instance_id}`)

	if (event.event === 'servers_updated') {
		if (isLinux && linuxRefreshCount.value >= MAX_LINUX_REFRESHES) return
		if (isLinux) linuxRefreshCount.value++

		await refreshAllWorlds()
	}

	await handleDefaultInstanceUpdateEvent(worlds.value, instance.value.id, event)
})

async function initWorldsTab() {
	const [resolvedProtocolVersion, resolvedGameVersions] = await Promise.all([
		get_instance_protocol_version(instance.value.id).catch(() => null),
		get_game_versions().catch(() => [] as GameVersion[]),
	])

	if (!worldsTabAlive) return

	protocolVersion.value = resolvedProtocolVersion
	gameVersions.value = resolvedGameVersions
	protocolVersionReady.value = true

	// Not only when the instance has worlds of its own: its server registered itself in
	// `serverData` before the protocol was known and is still waiting for its first ping.
	void refreshServers(worlds.value, serverData.value, protocolVersion.value)
}

void initWorldsTab()

async function refreshServer(address: string) {
	if (!serverData.value[address]) {
		serverData.value[address] = {
			refreshing: true,
		}
	}
	if (!protocolVersionReady.value) return
	await refreshServerData(serverData.value[address], protocolVersion.value, address)
}

// Ping the instance's server like the rest, and again whenever it starts, stops, or moves.
watch(
	() =>
		linkedServer.value && `${serverAddress(linkedServer.value)}|${linkedServer.value.power_state}`,
	() => {
		if (linkedServer.value) void refreshServer(serverAddress(linkedServer.value))
	},
	{ immediate: true },
)

// A hosted server's state is only known by pinging it, so each is pinged again whenever the list
// is fetched. The row keeps its last answer meanwhile instead of showing as loading.
watch(
	[hostedServersQuery.dataUpdatedAt, protocolVersionReady],
	async () => {
		if (!protocolVersionReady.value) return
		for (const { address } of hostedServers.value) {
			const answer: ServerData = { refreshing: true }
			void refreshServerData(answer, protocolVersion.value, address).then(() => {
				serverData.value[address] = answer
			})
		}
	},
	{ immediate: true },
)
async function refreshAllWorlds() {
	if (refreshingAll.value) {
		console.log(`Already refreshing, cancelling refresh.`)
		return
	}

	refreshingAll.value = true
	try {
		// Show loading on server rows immediately while the list refreshes
		for (const world of worlds.value) {
			if (world.type === 'server') {
				if (!serverData.value[world.address]) {
					serverData.value[world.address] = { refreshing: true }
				} else {
					serverData.value[world.address].refreshing = true
				}
			}
		}

		await queryClient.invalidateQueries({ queryKey: instanceKeys.worlds(instance.value.id) })
		await refreshServers(
			worlds.value,
			serverData.value,
			protocolVersion.value,
			protocolVersionReady.value,
		)
	} finally {
		refreshingAll.value = false
	}
}

async function addServer(server: ServerWorld) {
	worlds.value.push(server)
	sortWorlds(worlds.value)
	await refreshServer(server.address)
}

async function editServer(server: ServerWorld) {
	const index = worlds.value.findIndex((w) => w.type === 'server' && w.index === server.index)
	if (index !== -1) {
		const oldServer = worlds.value[index] as ServerWorld
		worlds.value[index] = server
		sortWorlds(worlds.value)
		if (oldServer.address !== server.address) {
			await refreshServer(server.address)
		}
	} else {
		handleError(new Error(`Error refreshing server, refreshing all worlds`))
		await refreshAllWorlds()
	}
}

async function removeServer(server: ServerWorld, scope: 'here' | 'all') {
	const instanceId = instance.value.id
	try {
		if (scope === 'here' && server.source === 'user_synced') {
			const updated = await set_synced_option(instanceId, 'multiplayer_servers', false)
			queryClient.setQueryData(instanceKeys.detail(instanceId), updated)
		}
		await remove_server_from_instance(instanceId, server.index)
	} catch (error) {
		handleError(error)
	} finally {
		await Promise.all([
			queryClient.invalidateQueries({ queryKey: instanceKeys.all }),
			queryClient.invalidateQueries({ queryKey: ['instance-synced-options'] }),
		])
		await refreshAllWorlds()
	}
}

async function confirmDesyncServer(server: ServerWorld, mode: DesyncServerMode) {
	if (!server.server_id) return
	await desync_server(instance.value.id, server.server_id, mode).catch(handleError)
	await refreshAllWorlds()
}

async function editWorld(path: string, name: string, removeIcon: boolean) {
	const world = worlds.value.find((world) => world.type === 'singleplayer' && world.path === path)
	if (world) {
		world.name = name
		if (removeIcon) {
			world.icon = undefined
		}
		sortWorlds(worlds.value)
	} else {
		handleError(new Error(`Error finding world in list, refreshing all worlds`))
		await refreshAllWorlds()
	}
}

async function deleteWorld(world: SingleplayerWorld) {
	await delete_world(instance.value.id, world.path).catch(handleError)
	worlds.value = worlds.value.filter((w) => w.type !== 'singleplayer' || w.path !== world.path)
}

function handleJoinError(err: Error) {
	handleSevereError(err, { instanceId: instance.value.id })
	startingInstance.value = false
	worldPlaying.value = undefined
}

async function joinWorld(world: World) {
	if (instance.value.quarantined) return
	console.log(`Joining world ${getWorldIdentifier(world)}`)
	startingInstance.value = true
	worldPlaying.value = world
	if (world.type === 'server') {
		const managedProjectId = instance.value.link?.project_id
		if (managedProjectId && isManagedServerWorld(world)) {
			await playServerProject(managedProjectId).catch(handleJoinError)
			trackEvent('InstanceStart', {
				loader: instance.value.loader,
				game_version: instance.value.game_version,
				source: 'WorldsPage',
			})
			startingInstance.value = false
			return
		}
		await start_join_server(instance.value.id, world.address).catch(handleJoinError)
		trackEvent('InstanceStart', {
			loader: instance.value.loader,
			game_version: instance.value.game_version,
			source: 'WorldsPage',
		})
	} else if (world.type === 'singleplayer') {
		await start_join_singleplayer_world(instance.value.id, world.path).catch(handleJoinError)
	}
	play()
	startingInstance.value = false
}

watch(
	() => playing.value,
	(playing) => {
		if (!playing) {
			worldPlaying.value = undefined

			setTimeout(async () => {
				for (const world of worlds.value) {
					if (world.type === 'singleplayer' && world.locked) {
						await refreshWorld(worlds.value, instance.value.id, world.path)
					}
				}
			}, 1000)
		}
	},
)

function worldsMatch(world: World, other: World | undefined) {
	if (world.type === 'server' && other?.type === 'server') {
		return world.address === other.address
	} else if (world.type === 'singleplayer' && other?.type === 'singleplayer') {
		return world.path === other.path
	}
	return false
}

const dedupedWorlds = computed(() => {
	const visibleWorlds: World[] = []
	const serverIndexByAddress = new Map<string, number>()

	for (const world of worlds.value) {
		if (world.type !== 'server') {
			visibleWorlds.push(world)
			continue
		}

		const addressKey = normalizeServerAddress(world.address) || `server-${world.index}`
		const existingIndex = serverIndexByAddress.get(addressKey)

		if (existingIndex == null) {
			serverIndexByAddress.set(addressKey, visibleWorlds.length)
			visibleWorlds.push(world)
			continue
		}

		// replace world with managed world if applicable
		const existingWorld = visibleWorlds[existingIndex]
		if (
			existingWorld?.type === 'server' &&
			!isManagedServerWorld(existingWorld) &&
			isManagedServerWorld(world)
		) {
			visibleWorlds[existingIndex] = world
		}
	}

	return visibleWorlds
})

const filterOptions = computed(() => {
	const options: { id: string; label: string }[] = []
	const listed = rows.value.map((row) => row.world)
	const hasSingleplayer = listed.some((x) => x.type === 'singleplayer')
	const hasServer = listed.some((x) => x.type === 'server')

	if (hasSingleplayer && hasServer) {
		options.push({ id: 'singleplayer', label: formatMessage(commonMessages.singleplayerLabel) })
	}

	if (hasServer) {
		const servers = listed.filter((x) => x.type === 'server')
		const hasVanilla = servers.some((x) => x.content_kind !== 'modpack')
		const hasModded = servers.some((x) => x.content_kind === 'modpack')
		if (hasVanilla && hasModded) {
			options.push({ id: 'vanilla', label: formatMessage(messages.vanillaFilter) })
			options.push({ id: 'modded', label: formatMessage(messages.moddedFilter) })
		}
		const hasOnline = servers.some((x) => !!serverData.value[x.address]?.status)
		const hasOffline = servers.some((x) => !serverData.value[x.address]?.status)
		if (hasOnline && hasOffline) {
			options.push({ id: 'online', label: formatMessage(messages.onlineFilter) })
			options.push({ id: 'offline', label: formatMessage(messages.offlineFilter) })
		}
	}

	return options
})

watch(filterOptions, (options) => {
	const validIds = new Set(options.map((opt) => opt.id))
	const cleaned = selectedFilters.value.filter((f) => validIds.has(f))
	if (cleaned.length !== selectedFilters.value.length) {
		selectedFilters.value = cleaned
	}
})

const filteredRows = computed(() =>
	rows.value.filter(({ world: x }) => {
		if (searchFilter.value && !worldNameMatchesQuery(x.name, searchFilter.value)) {
			return false
		}

		if (selectedFilters.value.length === 0) return true

		const hasSingleplayerFilter = selectedFilters.value.includes('singleplayer')
		const typeFilters = selectedFilters.value.filter((f) => f === 'vanilla' || f === 'modded')
		const statusFilters = selectedFilters.value.filter((f) => f === 'online' || f === 'offline')

		if (x.type === 'singleplayer') {
			return hasSingleplayerFilter || (typeFilters.length === 0 && statusFilters.length === 0)
		}

		if (hasSingleplayerFilter && typeFilters.length === 0 && statusFilters.length === 0) {
			return false
		}

		let passesType = true
		if (typeFilters.length > 0) {
			const isModded = x.content_kind === 'modpack'
			passesType =
				(typeFilters.includes('modded') && isModded) ||
				(typeFilters.includes('vanilla') && !isModded)
		}

		let passesStatus = true
		if (statusFilters.length > 0) {
			const isOnline = !!serverData.value[x.address]?.status
			passesStatus =
				(statusFilters.includes('online') && isOnline) ||
				(statusFilters.includes('offline') && !isOnline)
		}

		return passesType && passesStatus
	}),
)

const highlightedWorld = ref(route.query.highlight)

function promptToRemoveWorld(world: World): boolean {
	worldToRemove.value = world
	removeWorldModal.value?.show()
	return !!removeWorldModal.value
}

async function proceedRemoveWorld(world: World, scope: 'here' | 'all') {
	if (world.type === 'server') {
		await removeServer(world, scope)
	} else {
		await deleteWorld(world)
	}
	worldToRemove.value = null
}

onBeforeUnmount(() => {
	worldsTabAlive = false
})
</script>
