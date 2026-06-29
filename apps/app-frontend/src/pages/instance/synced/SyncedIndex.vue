<template>
	<div v-if="instance && server" class="flex h-full flex-col">
		<div class="shrink-0 p-6 pr-2 pb-4">
			<InstanceSettingsModal
				:key="instance.path"
				ref="settingsModal"
				:instance="instance"
				:offline="offline"
				@unlinked="fetchInstance"
			/>
			<ContentPageHeader>
				<template #icon>
					<Avatar
						:src="icon ?? undefined"
						:alt="instance.name"
						size="64px"
						:tint-by="instance.path"
					/>
				</template>
				<template #title>{{ instance.name }}</template>
				<template #stats>
					<div class="flex flex-wrap items-center gap-2">
						<div class="flex items-center gap-2 font-medium capitalize">
							{{ instance.loader }} {{ instance.game_version }}
						</div>
						<div class="h-1.5 w-1.5 rounded-full bg-surface-5"></div>
						<div class="flex items-center gap-1.5 font-medium text-secondary">
							<LinkIcon class="size-4" />
							Synced
						</div>
						<div class="h-1.5 w-1.5 rounded-full bg-surface-5"></div>
						<div
							class="flex items-center gap-1.5 font-medium"
							:class="powerState === 'running' ? 'text-green' : 'text-secondary'"
						>
							<span
								class="h-2 w-2 rounded-full"
								:class="powerState === 'running' ? 'bg-green' : 'bg-surface-5'"
							></span>
							{{ powerLabel }}
						</div>
					</div>
				</template>
				<template #actions>
					<div class="flex gap-2">
						<template v-if="hasPerm('server:power')">
							<ButtonStyled v-if="powerState === 'running'" color="red" size="large">
								<button @click="stopServer"><StopCircleIcon /> Stop server</button>
							</ButtonStyled>
							<ButtonStyled v-else color="blue" size="large">
								<button
									:disabled="powerState === 'starting' || powerState === 'stopping'"
									@click="startServer"
								>
									<ServerIcon />
									{{ powerState === 'starting' ? 'Starting' : 'Start server' }}
								</button>
							</ButtonStyled>
						</template>
						<ButtonStyled v-if="playing" color="red" size="large">
							<button @click="stopClient"><StopCircleIcon /> Stop</button>
						</ButtonStyled>
						<ButtonStyled v-else color="brand" size="large">
							<button :disabled="launching" @click="startClient">
								<PlayIcon />
								{{ launching ? 'Starting' : 'Play' }}
							</button>
						</ButtonStyled>
						<ButtonStyled v-if="hasPerm('instance:settings')" circular size="large">
							<button v-tooltip="'Instance settings'" @click="settingsModal?.show()">
								<SettingsIcon />
							</button>
						</ButtonStyled>
						<ButtonStyled type="transparent" circular size="large">
							<OverflowMenu
								:options="[
									{ id: 'open-folder', action: () => showProfileInFolder(instance.path) },
									{ id: 'copy-id', action: copyId },
									{ divider: true },
									...viewAsOptions,
								]"
							>
								<MoreVerticalIcon />
								<template #open-folder><FolderOpenIcon /> Open folder</template>
								<template #copy-id><ClipboardCopyIcon /> Copy server path</template>
								<template
									v-for="preset in PERMISSION_PRESET_ORDER"
									#[`view-as-${preset}`]
									:key="preset"
								>
									<CheckIcon v-if="permissionPreset === preset" />
									<EyeIcon v-else />
									View as {{ SYNCED_PERMISSION_PRESET_LABELS[preset] }}
								</template>
							</OverflowMenu>
						</ButtonStyled>
					</div>
				</template>
			</ContentPageHeader>
		</div>

		<div class="shrink-0 px-6">
			<NavTabs
				mode="local"
				:links="navLinks"
				:active-index="visibleActiveIndex"
				@tab-click="syncedTabController.selectTab"
			/>
		</div>

		<div class="min-h-0 flex-1 overflow-hidden p-6 pt-4">
			<NavTabContentTransition
				:content-key="activeTabKey"
				:direction="syncedTabSlideDirection"
				:visible="syncedTabContentVisible"
				@before-leave="syncedTabController.handleBeforeLeave"
				@after-leave="syncedTabController.handleAfterLeave"
				@after-enter="syncedTabController.handleAfterEnter"
				@enter-cancelled="syncedTabController.handleEnterCancelled"
				@leave-cancelled="syncedTabController.handleLeaveCancelled"
			>
				<Suspense>
					<component :is="activePage" :instance="instance" :offline="offline" :playing="playing" />
				</Suspense>
			</NavTabContentTransition>
		</div>

		<ServerSettingsModal />
	</div>
	<div v-else-if="loadError" class="flex min-h-full items-center justify-center p-6 text-contrast">
		<ErrorInformationCard
			title="Synced profile unavailable"
			:description="loadError.message"
			:icon="TriangleAlertIcon"
			icon-color="red"
		/>
	</div>
	<div v-else class="flex min-h-full items-center justify-center">
		<SpinnerIcon class="h-8 w-8 animate-spin text-contrast" />
	</div>
</template>

<script setup lang="ts">
import {
	BoxesIcon,
	CheckIcon,
	ClipboardCopyIcon,
	DatabaseBackupIcon,
	EyeIcon,
	FolderOpenIcon,
	HistoryIcon,
	LinkIcon,
	MoreVerticalIcon,
	PlayIcon,
	ServerIcon,
	SettingsIcon,
	SpinnerIcon,
	StopCircleIcon,
	TerminalSquareIcon,
	TriangleAlertIcon,
} from '@modrinth/assets'
import {
	Avatar,
	ButtonStyled,
	ContentPageHeader,
	ErrorInformationCard,
	injectNotificationManager,
	NavTabContentTransition,
	NavTabs,
	OverflowMenu,
	useNavTabContentController,
} from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { computed, defineAsyncComponent, provide, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import InstanceSettingsModal from '@/components/ui/modal/InstanceSettingsModal.vue'
import { get_by_profile_path } from '@/helpers/process'
import { get, kill, run } from '@/helpers/profile'
import type { GameInstance } from '@/helpers/types'
import { showProfileInFolder } from '@/helpers/utils.js'

import ServerSettingsModal from '../server/settings/ServerSettingsModal.vue'
import { useCoreServerRuntime } from '../server/use-core-server-runtime'
import { getLinkedServerId, getLinkedServerPath, setLinkedServerPath } from './use-synced-link'
import {
	provideSyncedPermissions,
	SYNCED_PERMISSION_PRESET_LABELS,
	SYNCED_PERMISSION_PRESETS,
	type SyncedPermission,
	type SyncedPermissionPreset,
} from './use-synced-permissions'
import { useSyncedRolePreset } from './use-synced-role'
import {
	provideSyncedSide,
	SYNCED_OPEN_CLIENT_SETTINGS,
	SYNCED_OPEN_SERVER_SETTINGS,
} from './use-synced-side'

const SyncedOverview = defineAsyncComponent(() => import('./SyncedOverview.vue'))
const SyncedContent = defineAsyncComponent(() => import('./SyncedContent.vue'))
const SyncedFiles = defineAsyncComponent(() => import('./SyncedFiles.vue'))
const SyncedBackups = defineAsyncComponent(() => import('./SyncedBackups.vue'))
const SyncedSync = defineAsyncComponent(() => import('./SyncedSync.vue'))
const SyncedSettings = defineAsyncComponent(() => import('./SyncedSettings.vue'))

const route = useRoute()
const { handleError } = injectNotificationManager()

provideSyncedSide('server')
const serverInstanceId = getLinkedServerId(route.params.id as string)
const cachedServerPath = ref(getLinkedServerPath(route.params.id as string))
const { powerState, startServer, stopServer, copyId, loadError, openSettings, server } =
	useCoreServerRuntime(serverInstanceId, cachedServerPath)

watch(
	server,
	(next) => {
		if (!next?.server_id) return
		cachedServerPath.value = next.server_id
		setLinkedServerPath(route.params.id as string, next.server_id)
	},
	{ immediate: true },
)

const permissionPreset = provideSyncedPermissions(useSyncedRolePreset(serverInstanceId).preset)
const permissions = computed(() => new Set(SYNCED_PERMISSION_PRESETS[permissionPreset.value]))
function hasPerm(permission: SyncedPermission) {
	return permissions.value.has(permission)
}
const PERMISSION_PRESET_ORDER: SyncedPermissionPreset[] = [
	'owner',
	'admin',
	'member',
	'client-only',
	'viewer',
]
const viewAsOptions = computed(() =>
	PERMISSION_PRESET_ORDER.map((preset) => ({
		id: `view-as-${preset}`,
		action: () => {
			permissionPreset.value = preset
		},
		remainOnClick: true,
	})),
)

const instance = ref<GameInstance>()
const offline = ref(!navigator.onLine)
const playing = ref(false)
const launching = ref(false)
const settingsModal = ref<InstanceType<typeof InstanceSettingsModal>>()

provide(SYNCED_OPEN_SERVER_SETTINGS, () => openSettings())
provide(SYNCED_OPEN_CLIENT_SETTINGS, () => settingsModal.value?.show())

const icon = computed(() =>
	instance.value?.icon_path ? convertFileSrc(instance.value.icon_path) : null,
)

const powerLabel = computed(() => {
	switch (powerState.value) {
		case 'running':
			return 'Server online'
		case 'starting':
			return 'Server starting'
		case 'stopping':
			return 'Server stopping'
		default:
			return 'Server offline'
	}
})

const pages = [
	SyncedOverview,
	SyncedContent,
	SyncedFiles,
	SyncedBackups,
	SyncedSync,
	SyncedSettings,
]
const navLinks = [
	{ label: 'Overview', href: 'overview', icon: TerminalSquareIcon },
	{ label: 'Content', href: 'content', icon: BoxesIcon },
	{ label: 'Files', href: 'files', icon: FolderOpenIcon },
	{ label: 'Backups', href: 'backups', icon: DatabaseBackupIcon },
	{ label: 'Sync', href: 'sync', icon: HistoryIcon },
	{ label: 'Settings', href: 'settings', icon: SettingsIcon },
]
const activeIndex = ref(0)
const syncedTabController = useNavTabContentController({
	activeIndex,
	changeTab: (index) => {
		activeIndex.value = index
	},
})
const visibleActiveIndex = syncedTabController.activeIndex
const syncedTabSlideDirection = syncedTabController.direction
const syncedTabContentVisible = syncedTabController.visible
const activeTabKey = computed(() => navLinks[activeIndex.value]?.href ?? activeIndex.value)
const activePage = computed(() => pages[activeIndex.value])

async function fetchInstance() {
	const next = await get(route.params.id as string).catch(handleError)
	instance.value = next ?? undefined
	await updatePlayState()
}

async function updatePlayState() {
	const running = await get_by_profile_path(route.params.id as string).catch(handleError)
	playing.value = Array.isArray(running) && running.length > 0
}

async function startClient() {
	launching.value = true
	try {
		await run(route.params.id as string)
		playing.value = true
	} catch (err) {
		handleError(err as Error)
	}
	launching.value = false
}

async function stopClient() {
	await kill(route.params.id as string).catch(handleError)
	playing.value = false
}

await fetchInstance()
</script>
