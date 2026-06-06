<script setup lang="ts">
import type { Archon, UploadState } from '@modrinth/api-client'
import {
	injectModrinthClient,
	provideModrinthClient,
	provideModrinthServerContext,
	ServerSettingsModal,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'

const props = defineProps<{
	name: string
	connectionUrl?: string
}>()

const queryClient = useQueryClient()
const realClient = injectModrinthClient()
const serverId = 'core-settings-preview'
const worldId = ref('default')
const modal = ref<InstanceType<typeof ServerSettingsModal> | null>(null)
const powerState = ref<Archon.Websocket.v0.PowerState>('stopped')
const uploadState = ref<UploadState>({
	isUploading: false,
	currentFileName: null,
	currentFileProgress: 0,
	uploadedBytes: 0,
	totalBytes: 0,
	completedFiles: 0,
	totalFiles: 0,
})
const server = ref(createServer())
const serverFull = computed(() => ({ ...server.value }))
const stats = ref({
	current: {
		cpu_percent: 0,
		ram_usage_bytes: 0,
		ram_total_bytes: 1024 * 1024 * 1024,
		storage_usage_bytes: 0,
		storage_total_bytes: 10 * 1024 * 1024 * 1024,
	},
	past: {
		cpu_percent: 0,
		ram_usage_bytes: 0,
		ram_total_bytes: 1024 * 1024 * 1024,
		storage_usage_bytes: 0,
		storage_total_bytes: 10 * 1024 * 1024 * 1024,
	},
	graph: { cpu: [0], ram: [0] },
})

provideModrinthClient(createMockClient())
provideModrinthServerContext({
	serverId,
	worldId,
	server,
	serverFull,
	currentUserPermissions: computed(() => 'SERVER_ADMIN'),
	isConnected: ref(true),
	isWsAuthIncorrect: ref(false),
	powerState,
	powerStateDetails: ref(undefined),
	isServerRunning: computed(() => powerState.value === 'running'),
	stats,
	uptimeSeconds: ref(0),
	isSyncingContent: ref(false),
	busyReasons: computed(() => []),
	fsAuth: ref(null),
	fsOps: ref([]),
	fsQueuedOps: ref([]),
	refreshFsAuth: async () => {},
	uploadState,
	cancelUpload: ref(null),
	activeOperations: computed(() => []),
	dismissOperation: async () => {},
})

watch(
	() => [props.name, props.connectionUrl],
	() => {
		server.value = createServer()
		seedServerCache()
	},
)

function createServer(): Archon.Servers.v0.Server {
	const [host = 'localhost', port = '16662'] = (props.connectionUrl ?? 'localhost:16662')
		.replace(/^https?:\/\//, '')
		.split(':')
	return {
		server_id: serverId,
		name: props.name,
		owner_id: 'local',
		net: { ip: '127.0.0.1', port: Number(port) || 16662, domain: host },
		game: 'Minecraft',
		backup_quota: 0,
		used_backup_quota: 0,
		status: 'available',
		suspension_reason: null,
		loader: 'Paper',
		loader_version: 'latest',
		mc_version: '1.21.5',
		upstream: null,
		sftp_username: 'core',
		sftp_password: 'mocked',
		sftp_host: host,
		datacenter: 'Local Core',
		notices: [],
		node: { instance: 'core-local' },
		flows: { intro: false },
		is_medal: false,
	} as Archon.Servers.v0.Server
}

function createMockClient() {
	return {
		...realClient,
		archon: {
			...realClient.archon,
			servers_v0: {
				...realClient.archon.servers_v0,
				get: async () => server.value,
				updateName: async (_id: string, name: string) => {
					server.value = { ...server.value, name }
				},
				checkSubdomainAvailability: async () => ({ available: true }),
				changeSubdomain: async (_id: string, domain: string) => {
					server.value = { ...server.value, net: { ...server.value.net, domain } }
				},
				reserveAllocation: async () => ({ name: 'Mock allocation', port: 24454 }),
				updateAllocation: async () => {},
				deleteAllocation: async () => {},
			},
			servers_v1: { ...realClient.archon.servers_v1, get: async () => serverFull.value },
			properties_v1: {
				...realClient.archon.properties_v1,
				getProperties: async () => ({}),
				updateProperties: async () => ({}),
			},
			options_v1: {
				...realClient.archon.options_v1,
				getStartup: async () => ({ jvm_args: '', java_version: 21 }),
				updateStartup: async () => ({}),
			},
			content_v1: {
				...realClient.archon.content_v1,
				getAddons: async () => ({ addons: [], modpack: null }),
				installContent: async () => ({}),
				repair: async () => ({}),
			},
		},
		labrinth: {
			...realClient.labrinth,
			billing_internal: {
				...realClient.labrinth.billing_internal,
				getSubscriptions: async () => [
					{ metadata: { type: 'pyro', id: serverId }, price_id: 'core-mock-price' },
				],
				getProducts: async () => [
					{
						prices: [{ id: 'core-mock-price' }],
						metadata: { type: 'pyro', cpu: 4, ram: 4096, swap: 1024, storage: 20480 },
					},
				],
			},
		},
	}
}

function seedServerCache() {
	queryClient.setQueryData(['servers', 'detail', serverId], server.value)
	queryClient.setQueryData(['servers', 'v1', 'detail', serverId], serverFull.value)
}

async function resolveViewer() {
	return { userId: 'local', userRole: 'admin' }
}

function show() {
	seedServerCache()
	modal.value?.show({ serverId })
}

function hide() {
	modal.value?.hide()
}

defineExpose({ show, hide })
</script>

<template>
	<ServerSettingsModal ref="modal" :resolve-viewer="resolveViewer" />
</template>
