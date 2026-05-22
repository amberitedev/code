<template>
	<div
		v-if="loadError"
		class="flex min-h-[calc(100vh-4rem)] items-center justify-center text-contrast"
	>
		<ErrorInformationCard
			title="Core server unavailable"
			description="Amberite Core is not responding. Make sure Core is running and try again."
			:icon="IssuesIcon"
			:action="reloadPage ? { label: 'Reload', onClick: reloadPage } : undefined"
		/>
	</div>
	<div
		v-else-if="server"
		data-pyro-server-manager-root
		class="relative mx-auto box-border flex w-full min-w-0 flex-col gap-4 px-6 pb-6 transition-all duration-300"
	>
		<ServerManageHeader
			class="server-stagger-item"
			:style="{ '--si': 0 }"
			:server="server"
			:server-project="null"
			:uptime-seconds="uptimeSeconds"
		>
			<template #actions>
				<div class="flex gap-2">
					<CoreServerActionButton :disabled="instance?.install_status === 'failed'" />
					<ButtonStyled circular size="large">
						<button v-tooltip="'Server settings'" @click="openSettings">
							<SettingsIcon />
						</button>
					</ButtonStyled>
					<CoreServerOverflowMenu :show-copy-id-action="showCopyIdAction" />
				</div>
			</template>
		</ServerManageHeader>

		<div
			data-pyro-navigation
			class="server-stagger-item isolate flex w-full select-none flex-col justify-between gap-4 overflow-auto md:flex-row md:items-center"
			:style="{ '--si': 1 }"
		>
			<NavTabs :links="navLinks" replace />
		</div>

		<div data-pyro-mount class="server-stagger-item h-full w-full flex-1" :style="{ '--si': 2 }">
			<div
				v-if="instance?.install_status === 'failed'"
				class="mx-auto mb-4 flex justify-between gap-2 rounded-2xl border-2 border-solid border-red bg-bg-red p-4 font-semibold text-contrast"
			>
				<div class="flex flex-row gap-4">
					<IssuesIcon class="hidden h-8 w-8 shrink-0 text-red sm:block" />
					<div class="flex flex-col gap-2 leading-[150%]">
						<div class="flex items-center gap-3">
							<IssuesIcon class="flex h-8 w-8 shrink-0 text-red sm:hidden" />
							<div class="flex gap-2 text-2xl font-bold">Installation error</div>
						</div>
						<div class="font-normal">
							This Core server failed to install. Try recreating it or checking the Core logs.
						</div>
					</div>
				</div>
			</div>

			<div
				v-if="!isConnected && !isReconnecting"
				data-pyro-server-ws-error
				class="mb-4 flex w-full flex-row items-center gap-4 rounded-2xl bg-bg-red p-4 text-contrast"
			>
				<IssuesIcon class="size-5 text-red" />
				Console connection unavailable.
			</div>

			<div
				v-if="isReconnecting"
				data-pyro-server-ws-reconnecting
				class="mb-4 flex w-full flex-row items-center gap-4 rounded-2xl bg-bg-orange p-4 text-sm text-contrast"
			>
				<LoaderCircleIcon class="h-5 w-5 animate-spin" />
				Hang on, we're reconnecting to your server.
			</div>

			<slot :on-reinstall="noop" :on-reinstall-failed="noop" />
		</div>
	</div>
</template>

<script setup lang="ts">
import type { CoreInstanceSummary } from '@amberite/amberite-api'
import type { Archon } from '@modrinth/api-client'
import {
	BoxesIcon,
	DatabaseBackupIcon,
	FolderOpenIcon,
	IssuesIcon,
	LayoutTemplateIcon,
	LoaderCircleIcon,
	SettingsIcon,
} from '@modrinth/assets'
import { computed, onUnmounted, ref, watch } from 'vue'

import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import ErrorInformationCard from '#ui/components/base/ErrorInformationCard.vue'
import NavTabs from '#ui/components/base/NavTabs.vue'
import ServerManageHeader from '#ui/components/servers/server-header/ServerManageHeader.vue'
import { injectCoreInstanceState } from '#ui/providers'

import CoreServerActionButton from './components/CoreServerActionButton.vue'
import CoreServerOverflowMenu from './components/CoreServerOverflowMenu.vue'
import { useCoreServerManageRuntime } from './runtime'

const props = withDefaults(
	defineProps<{
		serverId: string
		navHrefPrefix: string
		reloadPage?: () => void
		showCopyIdAction?: boolean
	}>(),
	{ reloadPage: undefined, showCopyIdAction: false },
)

const coreInstances = injectCoreInstanceState()
const snapshot = ref(coreInstances.snapshot)
const unlistenCoreInstances = coreInstances.subscribe((nextSnapshot) => {
	snapshot.value = nextSnapshot
})
const instance = computed<CoreInstanceSummary | null>(
	() => snapshot.value.instances.find((item) => item.id === props.serverId) ?? null,
)
const loadError = ref(false)
const isSyncingContent = ref(false)
const server = computed(() => {
	if (!props.serverId) return null
	return {
		server_id: props.serverId,
		name: instance.value?.name ?? 'Server',
		owner_id: '',
		net: { ip: '127.0.0.1', port: instance.value?.port ?? 25565, domain: null },
		game: 'java',
		backup_quota: 999,
		used_backup_quota: 0,
		status: instance.value?.install_status === 'installing' ? 'installing' : 'available',
		suspension_reason: null,
		loader: instance.value?.loader,
		loader_version: instance.value?.loader_version ?? '',
		mc_version: instance.value?.game_version,
		upstream: null,
		sftp_username: '',
		sftp_password: '',
		sftp_host: '',
		sftp_port: 22,
		datacenter: 'local',
		notices: [],
		node: { token: '', instance: '' },
		flows: { intro: false },
		is_medal: false,
	} as Archon.Servers.v0.Server
})

const { cleanupCoreRuntime, connectSocket, isConnected, isReconnecting, uptimeSeconds } =
	useCoreServerManageRuntime({
		serverId: computed(() => props.serverId),
		server,
		isSyncingContent,
		incrementUptimeLocally: true,
	})

const navLinks = computed(() => [
	{
		label: 'Overview',
		href: `${props.navHrefPrefix}`,
		icon: LayoutTemplateIcon,
		subpages: ['/server'],
	},
	{ label: 'Content', href: `${props.navHrefPrefix}/content`, icon: BoxesIcon },
	{ label: 'Files', href: `${props.navHrefPrefix}/files`, icon: FolderOpenIcon },
	{ label: 'Backups', href: `${props.navHrefPrefix}/backups`, icon: DatabaseBackupIcon },
])

const noop = () => {}
const openSettings = () => {}

async function refreshInstance() {
	if (!props.serverId) return
	loadError.value = false
	try {
		await coreInstances.refresh()
	} catch (error) {
		console.error('[core/server-manage] Failed to load instance:', error)
		loadError.value = true
	}
}

watch(
	() => props.serverId,
	async (serverId, previousId) => {
		if (previousId && previousId !== serverId) cleanupCoreRuntime()
		await refreshInstance()
		if (serverId) void connectSocket(serverId)
	},
	{ immediate: true },
)

onUnmounted(() => {
	unlistenCoreInstances()
	cleanupCoreRuntime()
})
</script>

<style scoped lang="scss">
.server-stagger-item {
	animation: server-stagger-in 0.45s ease both;
	animation-delay: calc(var(--si, 0) * 0.08s);
}

@keyframes server-stagger-in {
	from {
		opacity: 0;
		transform: translateY(8px);
	}
	to {
		opacity: 1;
		transform: translateY(0);
	}
}
</style>
