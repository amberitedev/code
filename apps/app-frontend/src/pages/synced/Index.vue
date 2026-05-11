<template>
	<div v-if="instance" :class="{ 'flex h-full flex-col': isFixedRender }">
		<div :class="['p-6 pr-2 pb-4', { 'shrink-0': isFixedRender }]">
			<ExportModal ref="exportModal" :instance="instance" />
			<InstanceSettingsModal
				:key="instance.path"
				ref="settingsModal"
				:instance="instance"
				:offline="offline"
				@unlinked="fetchInstance"
			/>
			<UpdateToPlayModal ref="updateToPlayModal" :instance="instance" />
			<ContentPageHeader>
				<template #icon>
					<Avatar
						:src="icon ? icon : undefined"
						:alt="instance.name"
						size="64px"
						:tint-by="instance.path"
					/>
				</template>
				<template #title>
					{{ instance.name }}
				</template>
				<template #stats>
					<div class="flex items-center flex-wrap gap-2">
						<div class="flex items-center gap-2 capitalize font-medium">
							{{ instance.loader }} {{ instance.game_version }}
						</div>
						<div class="w-1.5 h-1.5 rounded-full bg-surface-5"></div>
						<div class="flex items-center gap-2 font-medium">
							<template v-if="timePlayed > 0">{{ timePlayedHumanized }}</template>
							<template v-else>Never played</template>
						</div>
					</div>
				</template>
				<template #actions>
					<div class="flex gap-2">
						<ButtonStyled
							v-if="['installing', 'pack_installing', 'pack_installed', 'not_installed', 'minecraft_installing'].includes(instance.install_stage)"
							color="brand"
							size="large"
						>
							<button disabled>Installing...</button>
						</ButtonStyled>
						<ButtonStyled
							v-else-if="instance.install_stage !== 'installed'"
							color="brand"
							size="large"
						>
							<button @click="repairInstance()">
								<DownloadIcon />
								Repair
							</button>
						</ButtonStyled>
						<ButtonStyled v-else-if="loading && !playing" color="brand" size="large">
							<button disabled>Starting...</button>
						</ButtonStyled>
						<JoinedButtons
							v-else
							:color="playing ? 'red' : 'brand'"
							size="large"
							:actions="playActions"
							:primary-disabled="stopping"
						/>
						<ButtonStyled circular size="large">
							<button v-tooltip="'Instance settings'" @click="settingsModal?.show()">
								<SettingsIcon />
							</button>
						</ButtonStyled>
						<ButtonStyled type="transparent" circular size="large">
							<OverflowMenu
								:options="[
									{ id: 'open-folder', action: () => { if (instance) showProfileInFolder(instance.path) } },
									{ id: 'export-mrpack', action: () => exportModal?.show() },
								]"
							>
								<MoreVerticalIcon />
								<template #open-folder><FolderOpenIcon /> Open folder</template>
								<template #export-mrpack><PackageIcon /> Export modpack</template>
							</OverflowMenu>
						</ButtonStyled>
					</div>
				</template>
			</ContentPageHeader>
		</div>
		<div :class="['px-6', { 'shrink-0': isFixedRender }]">
			<NavTabs :links="tabs" />
		</div>
		<div :class="['p-6 pt-4', { 'min-h-0 flex-1 overflow-y-auto': isFixedRender }]">
			<ServerSetup :core-instance-id="coreInstanceId">
				<RouterView
					v-if="route.path.startsWith('/synced')"
					v-slot="{ Component }"
					:key="instance.path"
				>
					<template v-if="Component">
						<Suspense :key="instance.path">
							<component
								:is="Component"
								:instance="instance"
								:options="options"
								:offline="offline"
								:playing="playing"
								:installed="instance.install_stage !== 'installed'"
								:open-settings="() => settingsModal?.show(1)"
								@play="updatePlayState"
								@stop="() => stopInstance('SyncedSubpage')"
							/>
						</Suspense>
					</template>
				</RouterView>
			</ServerSetup>
		</div>
		<ContextMenu ref="options" @option-clicked="handleOptionsClick">
			<template #play><PlayIcon /> Play</template>
			<template #stop><StopCircleIcon /> Stop</template>
			<template #add_content><PlusIcon /> Add content</template>
			<template #open_folder><FolderOpenIcon /> Open folder</template>
			<template #copy_path><ClipboardCopyIcon /> Copy path</template>
		</ContextMenu>
	</div>
</template>

<script setup lang="ts">
import { CoreApiClient } from '@amberite/core-client'
import {
	BoxesIcon,
	ClipboardCopyIcon,
	DownloadIcon,
	FolderOpenIcon,
	GlobeIcon,
	MoreVerticalIcon,
	PackageIcon,
	PlayIcon,
	PlusIcon,
	ServerIcon,
	SettingsIcon,
	StopCircleIcon,
	TerminalSquareIcon,
} from '@modrinth/assets'
import {
	Avatar,
	ButtonStyled,
	ContentPageHeader,
	injectNotificationManager,
	JoinedButtons,
	NavTabs,
	OverflowMenu,
	provideCoreClient,
} from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import dayjs from 'dayjs'
import duration from 'dayjs/plugin/duration'
import relativeTime from 'dayjs/plugin/relativeTime'
import { computed, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import ContextMenu from '@/components/ui/ContextMenu.vue'
import ExportModal from '@/components/ui/ExportModal.vue'
import InstanceSettingsModal from '@/components/ui/modal/InstanceSettingsModal.vue'
import UpdateToPlayModal from '@/components/ui/modal/UpdateToPlayModal.vue'
import { trackEvent } from '@/helpers/analytics'
import { core_get_url } from '@/helpers/core'
import { process_listener, profile_listener } from '@/helpers/events'
import { get_by_profile_path } from '@/helpers/process'
import { finish_install, get, get_full_path, kill, run } from '@/helpers/profile'
import type { GameInstance } from '@/helpers/types'
import { showProfileInFolder } from '@/helpers/utils.js'
import { handleSevereError } from '@/store/error.js'
import { useBreadcrumbs } from '@/store/state'

import ServerSetup from './ServerSetup.vue'

dayjs.extend(duration)
dayjs.extend(relativeTime)

const { handleError } = injectNotificationManager()
const route = useRoute()
const router = useRouter()
const breadcrumbs = useBreadcrumbs()

const offline = ref(!navigator.onLine)
window.addEventListener('offline', () => { offline.value = true })
window.addEventListener('online', () => { offline.value = false })

const instance = ref<GameInstance>()
const playing = ref(false)
const loading = ref(false)
const stopping = ref(false)
const exportModal = ref<InstanceType<typeof ExportModal>>()
const settingsModal = ref<InstanceType<typeof InstanceSettingsModal>>()
const updateToPlayModal = ref<InstanceType<typeof UpdateToPlayModal>>()
const options = ref<InstanceType<typeof ContextMenu> | null>(null)

async function fetchInstance() {
	instance.value = await get(route.params.id as string).catch(handleError)
	if (instance.value) {
		breadcrumbs.setName(
			'SyncedInstance',
			instance.value.name.length > 40 ? instance.value.name.substring(0, 40) + '...' : instance.value.name,
		)
		breadcrumbs.setContext({ name: instance.value.name, link: route.path, query: route.query })
	}
	await updatePlayState()
}

await fetchInstance()

const coreInstanceId = instance.value?.core_instance_id ?? (route.params.id as string)
const baseUrl = await core_get_url()
provideCoreClient(new CoreApiClient(baseUrl))

watch(
	() => route.params.id,
	async () => {
		if (route.params.id && route.path.startsWith('/synced')) {
			await fetchInstance()
		}
	},
)

const basePath = computed(() => `/synced/${encodeURIComponent(route.params.id as string)}`)

const renderMode = computed<'scroll' | 'fixed'>(() =>
	route.meta.renderMode === 'fixed' ? 'fixed' : 'scroll',
)
const isFixedRender = computed(() => renderMode.value === 'fixed')

const tabs = computed(() => [
	{ label: 'Content', href: `${basePath.value}`, icon: BoxesIcon },
	{ label: 'Files', href: `${basePath.value}/files`, icon: FolderOpenIcon },
	{ label: 'Worlds', href: `${basePath.value}/worlds`, icon: GlobeIcon },
	{ label: 'Logs', href: `${basePath.value}/logs`, icon: TerminalSquareIcon },
	{ label: 'Console', href: `${basePath.value}/console`, icon: ServerIcon },
	{ label: 'Backups', href: `${basePath.value}/backups`, icon: PackageIcon },
])

async function updatePlayState() {
	if (!route.params.id) return
	const procs = await get_by_profile_path(route.params.id as string).catch(handleError)
	playing.value = Array.isArray(procs) && procs.length > 0
}

const startInstance = async (context: string) => {
	if (!instance.value) return
	if (updateToPlayModal.value?.hasUpdate) {
		updateToPlayModal.value.show(instance.value)
		return
	}
	loading.value = true
	try {
		await run(route.params.id as string)
		playing.value = true
	} catch (err) {
		handleSevereError(err, { profilePath: route.params.id as string })
	}
	loading.value = false
	trackEvent('InstanceStart', { loader: instance.value.loader, game_version: instance.value.game_version, source: context })
}

const stopInstance = async (context: string) => {
	stopping.value = true
	await kill(route.params.id as string).catch(handleError)
	stopping.value = false
	playing.value = false
	if (!instance.value) return
	trackEvent('InstanceStop', { loader: instance.value.loader, game_version: instance.value.game_version, source: context })
}

const playActions = computed(() => {
	if (playing.value) {
		return [{ id: 'stop', label: stopping.value ? 'Stopping...' : 'Stop', icon: StopCircleIcon, action: () => stopInstance('SyncedPage') }]
	}
	return [{ id: 'play', label: 'Play', icon: PlayIcon, action: () => startInstance('SyncedPage') }]
})

const repairInstance = async () => {
	await finish_install(instance.value).catch(handleError)
}

const icon = computed(() =>
	instance.value?.icon_path ? convertFileSrc(instance.value.icon_path) : null,
)

const timePlayed = computed(() =>
	instance.value ? instance.value.recent_time_played + instance.value.submitted_time_played : 0,
)

const timePlayedHumanized = computed(() => {
	const dur = dayjs.duration(timePlayed.value, 'seconds')
	const hours = Math.floor(dur.asHours())
	if (hours >= 1) return hours + ' hour' + (hours > 1 ? 's' : '')
	const minutes = Math.floor(dur.asMinutes())
	if (minutes >= 1) return minutes + ' minute' + (minutes > 1 ? 's' : '')
	return timePlayed.value + ' second' + (timePlayed.value !== 1 ? 's' : '')
})

const handleOptionsClick = async (args: { option: string; item: unknown }) => {
	switch (args.option) {
		case 'play':
			await startInstance('SyncedPageContextMenu')
			break
		case 'stop':
			await stopInstance('SyncedPageContextMenu')
			break
		case 'add_content':
			await router.push({ path: `/browse/${instance.value?.loader === 'vanilla' ? 'datapack' : 'mod'}`, query: { i: route.params.id } })
			break
		case 'open_folder':
			if (instance.value) await showProfileInFolder(instance.value.path)
			break
		case 'copy_path': {
			if (instance.value) {
				const fullPath = await get_full_path(instance.value.path)
				await navigator.clipboard.writeText(fullPath)
			}
			break
		}
	}
}

const unlistenProfiles = await profile_listener(
	async (event: { profile_path_id: string; event: string }) => {
		if (event.profile_path_id !== route.params.id) return
		if (event.event === 'removed' || route.path === '/') {
			if (route.path !== '/') await router.push({ path: '/' })
			return
		}
		instance.value = await get(route.params.id as string).catch((err) => {
			if (String(err).includes('not managed')) { router.push({ path: '/' }); return undefined }
			return handleError(err)
		})
	},
)

const unlistenProcesses = await process_listener(
	(e: { event: string; profile_path_id: string }) => {
		if (e.event === 'finished' && e.profile_path_id === route.params.id) {
			playing.value = false
		}
	},
)

onUnmounted(() => {
	unlistenProfiles()
	unlistenProcesses()
})
</script>
