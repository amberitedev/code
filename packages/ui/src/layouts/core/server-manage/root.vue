<template>
	<div class="flex min-h-full flex-col gap-6">
		<header class="flex flex-col gap-4 rounded-2xl bg-surface-2 p-5">
			<div class="flex flex-wrap items-center justify-between gap-3">
				<div>
					<p class="text-sm font-semibold text-secondary">Amberite Core</p>
					<h1 class="text-3xl font-bold text-contrast">{{ instance?.name ?? 'Server' }}</h1>
					<p class="text-sm text-secondary">{{ subtitle }}</p>
				</div>
				<div class="flex gap-2">
					<button
						class="rounded-full bg-green px-4 py-2 font-semibold text-white"
						@click="startServer"
					>
						Start
					</button>
					<button
						class="rounded-full bg-surface-4 px-4 py-2 font-semibold text-contrast"
						@click="stopServer"
					>
						Stop
					</button>
					<button
						class="rounded-full bg-surface-4 px-4 py-2 font-semibold text-contrast"
						@click="restartServer"
					>
						Restart
					</button>
				</div>
			</div>
			<NavTabs :links="navLinks" replace />
		</header>

		<div v-if="loadError" class="rounded-2xl bg-red-highlight p-4 text-red">{{ loadError }}</div>
		<slot v-else :on-reinstall="noop" :on-reinstall-failed="noop" />
	</div>
</template>

<script setup lang="ts">
import type { CoreInstance } from '@amberite/api-lib'
import type { Archon } from '@modrinth/api-client'
import { computed, onUnmounted, ref, watch } from 'vue'

import NavTabs from '#ui/components/base/NavTabs.vue'
import { injectCoreClient } from '#ui/providers'

import { useCoreServerManageRuntime } from './runtime'

const props = withDefaults(
	defineProps<{
		serverId: string
		navHrefPrefix: string
		reloadPage?: () => void
	}>(),
	{ reloadPage: undefined },
)

const coreClient = injectCoreClient()
const instance = ref<CoreInstance | null>(null)
const loadError = ref<string | null>(null)
const isSyncingContent = ref(false)
const server = computed(
	() =>
		({
			id: props.serverId,
			name: instance.value?.name ?? 'Server',
			loader: instance.value?.loader,
			mc_version: instance.value?.game_version,
			status: instance.value?.status,
		}) as unknown as Archon.Servers.v0.Server,
)

const { cleanupCoreRuntime, connectSocket } = useCoreServerManageRuntime({
	serverId: computed(() => props.serverId),
	server,
	isSyncingContent,
	incrementUptimeLocally: true,
})

const subtitle = computed(() => {
	if (!instance.value) return 'Loading Core server details...'
	return `${instance.value.loader} ${instance.value.game_version} · Port ${instance.value.port}`
})

const navLinks = computed(() => [
	{ label: 'Overview', href: `${props.navHrefPrefix}` },
	{ label: 'Console', href: `${props.navHrefPrefix}/console` },
	{ label: 'Content', href: `${props.navHrefPrefix}/content` },
	{ label: 'Files', href: `${props.navHrefPrefix}/files` },
	{ label: 'Backups', href: `${props.navHrefPrefix}/backups` },
])

const noop = () => {}

async function refreshInstance() {
	if (!props.serverId) return
	loadError.value = null
	try {
		instance.value = await coreClient.getInstance(props.serverId)
	} catch (error) {
		console.error('[core/server-manage] Failed to load instance:', error)
		loadError.value = 'Could not load this Core server.'
	}
}

async function runAction(action: 'start' | 'stop' | 'restart') {
	if (!props.serverId) return
	await coreClient[action](props.serverId)
	await refreshInstance()
}

const startServer = () => void runAction('start')
const stopServer = () => void runAction('stop')
const restartServer = () => void runAction('restart')

watch(
	() => props.serverId,
	async (serverId, previousId) => {
		if (previousId && previousId !== serverId) cleanupCoreRuntime()
		await refreshInstance()
		if (serverId) void connectSocket(serverId)
	},
	{ immediate: true },
)

onUnmounted(() => cleanupCoreRuntime())
</script>
