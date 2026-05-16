<template>
	<div
		v-if="coreError"
		class="flex min-h-[calc(100vh-4rem)] items-center justify-center text-contrast"
	>
		<ErrorInformationCard
			title="Core Unavailable"
			description="Amberite Core is not responding. Make sure Core is running and try again."
			:icon="IssuesIcon"
		/>
	</div>
	<div v-else-if="instance" :class="{ 'flex h-full flex-col': isFixedRender }">
		<div
			:class="['p-6 pr-2 pb-4', { 'shrink-0': isFixedRender }]"
			@contextmenu.prevent.stop="(event) => handleRightClick(event)"
		>
			<ExportModal ref="exportModal" :instance="instance" />
			<InstanceSettingsModal
				:key="instance.path"
				ref="settingsModal"
				:instance="instance"
				:offline="offline"
				@unlinked="fetchInstance"
			/>
			<UpdateToPlayModal ref="updateToPlayModal" />
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
						<template v-if="!isServerInstance">
							<div class="flex items-center gap-2 capitalize font-medium">
								{{ instance.loader }} {{ instance.game_version }}
							</div>

							<div class="w-1.5 h-1.5 rounded-full bg-surface-5"></div>

							<div class="flex items-center gap-2 font-medium">
								<template v-if="timePlayed > 0">
									{{ timePlayedHumanized }}
								</template>
								<template v-else> Never played </template>
							</div>
						</template>

						<template v-else>
							<template v-if="loadingServerPing">
								<ServerOnlinePlayers
									v-if="playersOnline !== undefined"
									:online="playersOnline"
									:status-online="statusOnline"
									hide-label
								/>
								<ServerRecentPlays :recent-plays="recentPlays ?? 0" hide-label />
								<div
									v-if="
										(playersOnline !== undefined || recentPlays !== undefined) &&
										(minecraftServer?.region || ping)
									"
									class="w-1.5 h-1.5 rounded-full bg-surface-5"
								></div>
								<ServerPing v-if="ping" :ping="ping" />
							</template>

							<ServerRegion v-if="minecraftServer?.region" :region="minecraftServer?.region" />

							<div
								v-if="minecraftServer?.region || ping"
								class="w-1.5 h-1.5 rounded-full bg-surface-5"
							></div>

							<div
								v-if="linkedProjectV3"
								class="flex gap-1.5 items-center font-medium text-primary"
							>
								Linked to
								<Avatar
									:src="linkedProjectV3.icon_url"
									:alt="linkedProjectV3.name"
									:tint-by="instance.path"
									size="24px"
								/>
								<router-link
									:to="`/project/${linkedProjectV3.slug ?? linkedProjectV3.id}`"
									class="hover:underline text-primary truncate"
								>
									{{ linkedProjectV3.name }}
								</router-link>
							</div>
						</template>
					</div>
				</template>
				<template #actions>
					<div class="flex gap-2">
						<ButtonStyled
							v-if="
								[
									'installing',
									'pack_installing',
									'pack_installed',
									'not_installed',
									'minecraft_installing',
								].includes(instance.install_stage)
							"
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
									{
										id: 'open-folder',
										action: () => {
											if (instance) showProfileInFolder(instance.path)
										},
									},
									{
										id: 'export-mrpack',
										action: () => exportModal?.show(),
									},
								]"
							>
								<MoreVerticalIcon />
								<template #open-folder> <FolderOpenIcon /> Open folder </template>
								<template #export-mrpack> <PackageIcon /> Export modpack </template>
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
					:key="route.fullPath"
				>
					<template v-if="Component">
						<Suspense :key="route.fullPath">
							<component
								:is="Component"
								:instance="instance"
								:options="options"
								:offline="offline"
								:playing="playing"
								:installed="instance.install_stage !== 'installed'"
								:is-server-instance="isServerInstance"
								:open-settings="() => settingsModal?.show(1)"
								@play="updatePlayState"
								@stop="() => stopInstance('SyncedSubpage')"
							></component>
						</Suspense>
					</template>
				</RouterView>
			</ServerSetup>
		</div>
		<ContextMenu ref="options" @option-clicked="handleOptionsClick">
			<template #play> <PlayIcon /> Play </template>
			<template #stop> <StopCircleIcon /> Stop </template>
			<template #add_content> <PlusIcon /> Add content </template>
			<template #edit> <EditIcon /> Edit </template>
			<template #copy_path> <ClipboardCopyIcon /> Copy path </template>
			<template #open_folder> <FolderOpenIcon /> Open folder </template>
			<template #copy_link> <ClipboardCopyIcon /> Copy link </template>
			<template #open_link> <GlobeIcon /> Open in Modrinth <ExternalIcon /> </template>
			<template #copy_names><EditIcon />Copy names</template>
			<template #copy_slugs><HashIcon />Copy slugs</template>
			<template #copy_links><GlobeIcon />Copy links</template>
			<template #toggle><EditIcon />Toggle selected</template>
			<template #disable><XIcon />Disable selected</template>
			<template #enable><CheckCircleIcon />Enable selected</template>
			<template #hide_show><EyeIcon />Show/Hide unselected</template>
			<template #update_all
				><UpdatedIcon />Update {{ selected.length > 0 ? 'selected' : 'all' }}</template
			>
			<template #filter_update><UpdatedIcon />Select Updatable</template>
		</ContextMenu>
	</div>
	<div v-else class="flex min-h-[calc(100vh-4rem)] items-center justify-center text-contrast">
		<ErrorInformationCard
			title="Instance Unavailable"
			description="This instance could not be loaded. It may have been deleted or is temporarily unavailable."
			:icon="IssuesIcon"
		/>
	</div>
</template>
<script setup lang="ts">
import { CoreApiClient } from '@amberite/api-lib'
import type { Labrinth } from '@modrinth/api-client'
import {
	BoxesIcon,
	CheckCircleIcon,
	ClipboardCopyIcon,
	DownloadIcon,
	EditIcon,
	ExternalIcon,
	EyeIcon,
	FolderOpenIcon,
	GlobeIcon,
	HashIcon,
	IssuesIcon,
	MoreVerticalIcon,
	PackageIcon,
	PlayIcon,
	PlusIcon,
	ServerIcon,
	SettingsIcon,
	StopCircleIcon,
	TerminalSquareIcon,
	UpdatedIcon,
	XIcon,
} from '@modrinth/assets'
import {
	Avatar,
	ButtonStyled,
	ContentPageHeader,
	ErrorInformationCard,
	injectNotificationManager,
	JoinedButtons,
	NavTabs,
	OverflowMenu,
	provideCoreClient,
	ServerOnlinePlayers,
	ServerPing,
	ServerRecentPlays,
	ServerRegion,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { convertFileSrc } from '@tauri-apps/api/core'
import dayjs from 'dayjs'
import duration from 'dayjs/plugin/duration'
import relativeTime from 'dayjs/plugin/relativeTime'
import { computed, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { getDesktopAdapter } from '@/adapters/desktop'
import ContextMenu from '@/components/ui/ContextMenu.vue'
import ExportModal from '@/components/ui/ExportModal.vue'
import InstanceSettingsModal from '@/components/ui/modal/InstanceSettingsModal.vue'
import UpdateToPlayModal from '@/components/ui/modal/UpdateToPlayModal.vue'
import { trackEvent } from '@/helpers/analytics'
import { get_project_v3 } from '@/helpers/cache.js'
import { process_listener, profile_listener } from '@/helpers/events'
import { get_by_profile_path } from '@/helpers/process'
import { finish_install, get, get_full_path, kill, run } from '@/helpers/profile'
import type { GameInstance } from '@/helpers/types'
import { showProfileInFolder } from '@/helpers/utils.js'
import { get_server_status, refreshWorlds } from '@/helpers/worlds'
import { injectServerInstall } from '@/providers/server-install'
import { handleSevereError } from '@/store/error.js'
import { useBreadcrumbs } from '@/store/state'

import ServerSetup from './ServerSetup.vue'

dayjs.extend(duration)
dayjs.extend(relativeTime)

const { handleError } = injectNotificationManager()
const { playServerProject } = injectServerInstall()
const queryClient = useQueryClient()
const route = useRoute()
const router = useRouter()
const breadcrumbs = useBreadcrumbs()

const offline = ref(!navigator.onLine)
window.addEventListener('offline', () => {
	offline.value = true
})
window.addEventListener('online', () => {
	offline.value = false
})

const instance = ref<GameInstance>()
const coreError = ref(false)
const playing = ref(false)
const loading = ref(false)
const stopping = ref(false)
const exportModal = ref<InstanceType<typeof ExportModal>>()
const updateToPlayModal = ref<InstanceType<typeof UpdateToPlayModal>>()

const isServerInstance = computed(
	() => instance.value?.kind === 'server' || instance.value?.kind === 'synced',
)
const linkedProjectV3 = ref<Labrinth.Projects.v3.Project>()
const selected = ref<unknown[]>([])

const minecraftServer = computed(() => linkedProjectV3.value?.minecraft_server)
const javaServerPingData = computed(() => linkedProjectV3.value?.minecraft_java_server?.ping?.data)
const statusOnline = computed(() => !!javaServerPingData.value)
const recentPlays = computed(
	() => linkedProjectV3.value?.minecraft_java_server?.verified_plays_2w ?? undefined,
)
const playersOnline = ref<number | undefined>(undefined)
const ping = ref<number | undefined>(undefined)
const loadingServerPing = ref(false)

async function fetchInstance() {
	coreError.value = false
	linkedProjectV3.value = undefined
	ping.value = undefined
	playersOnline.value = undefined
	loadingServerPing.value = false

	instance.value = await get(route.params.id as string).catch(handleError)

	if (instance.value?.kind === 'server') {
		await router.replace(`/server/${encodeURIComponent(route.params.id as string)}`)
		return
	}

	if (instance.value?.kind === 'synced' && !instance.value.core_instance_id) {
		coreError.value = true
		return
	}

	if (instance.value) {
		breadcrumbs.setName(
			'SyncedInstance',
			instance.value.name.length > 40
				? instance.value.name.substring(0, 40) + '...'
				: instance.value.name,
		)
		breadcrumbs.setContext({
			name: instance.value.name,
			link: route.path,
			query: route.query,
		})
	}

	if (!offline.value && instance.value?.linked_data && instance.value.linked_data.project_id) {
		try {
			linkedProjectV3.value = await get_project_v3(
				instance.value.linked_data.project_id,
				'must_revalidate',
			)
		} catch (error) {
			handleError(error as Error)
		}
	}

	fetchDeferredData()

	if (instance.value) {
		queryClient.prefetchQuery({
			queryKey: ['worlds', instance.value.path],
			queryFn: () => refreshWorlds(instance.value!.path),
			staleTime: 30_000,
		})
	}
}

function fetchDeferredData() {
	const serverAddress = linkedProjectV3.value?.minecraft_java_server?.address
	if (isServerInstance.value && serverAddress) {
		get_server_status(serverAddress)
			.then((status) => {
				playersOnline.value = status.players?.online
				ping.value = status.ping
			})
			.catch((error) => {
				console.error(`Failed to fetch server status for ${serverAddress}:`, error)
			})
			.finally(() => {
				loadingServerPing.value = true
			})
	} else {
		loadingServerPing.value = true
	}

	updatePlayState()
}

async function updatePlayState() {
	if (!route.params.id) return
	const runningProcesses = await get_by_profile_path(route.params.id as string).catch(handleError)
	playing.value = Array.isArray(runningProcesses) && runningProcesses.length > 0
}

let _unmounted = false
const unlistenProfiles = ref<(() => void) | undefined>()
const unlistenProcesses = ref<(() => void) | undefined>()

try {
	const adapter = getDesktopAdapter()
	provideCoreClient(new CoreApiClient(adapter))
} catch {
	coreError.value = true
}

onUnmounted(() => {
	_unmounted = true
	unlistenProfiles.value?.()
	unlistenProcesses.value?.()
})

await fetchInstance()

const coreInstanceId = computed(() => instance.value?.core_instance_id ?? '')

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

const options = ref<InstanceType<typeof ContextMenu> | null>(null)
const settingsModal = ref<InstanceType<typeof InstanceSettingsModal>>()

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

	trackEvent('InstanceStart', {
		loader: instance.value.loader,
		game_version: instance.value.game_version,
		source: context,
	})
}

const stopInstance = async (context: string) => {
	stopping.value = true
	await kill(route.params.id as string).catch(handleError)
	stopping.value = false
	playing.value = false

	if (!instance.value) return
	trackEvent('InstanceStop', {
		loader: instance.value.loader,
		game_version: instance.value.game_version,
		source: context,
	})
}

const handlePlayServer = async () => {
	if (!instance.value?.linked_data?.project_id) return
	loading.value = true
	try {
		await playServerProject(instance.value.linked_data.project_id)
	} finally {
		await updatePlayState()
		loading.value = false
	}
}

// TODO: AMBERITE - wire serverRunning to Core server state
const serverRunning = ref(false)

const playActions = computed(() => {
	if (playing.value) {
		return [
			{
				id: 'stop',
				label: stopping.value ? 'Stopping...' : 'Stop',
				icon: StopCircleIcon,
				action: () => stopInstance('SyncedPage'),
			},
			serverRunning.value
				? { id: 'stop_server', label: 'Stop server', icon: ServerIcon, action: () => {} }
				: { id: 'start_server', label: 'Start server', icon: ServerIcon, action: () => {} },
		]
	}
	if (serverRunning.value) {
		return [
			{ id: 'join', label: 'Join', icon: PlayIcon, action: () => handlePlayServer() },
			{ id: 'stop_server', label: 'Stop server', icon: ServerIcon, action: () => {} },
		]
	}
	return [
		{ id: 'play', label: 'Play', icon: PlayIcon, action: () => startInstance('SyncedPage') },
		{ id: 'start_server', label: 'Start server', icon: ServerIcon, action: () => {} },
	]
})

const repairInstance = async () => {
	await finish_install(instance.value).catch(handleError)
}

const handleRightClick = (event: MouseEvent) => {
	const baseOptions = [
		{ name: 'add_content' },
		{ type: 'divider' },
		{ name: 'open_folder' },
		{ name: 'copy_path' },
	]

	options.value?.showMenu(
		event,
		instance.value,
		playing.value
			? [{ name: 'stop', color: 'danger' }, ...baseOptions]
			: [{ name: 'play', color: 'primary' }, ...baseOptions],
	)
}

const handleOptionsClick = async (args: { option: string; item: unknown }) => {
	switch (args.option) {
		case 'play':
			await startInstance('SyncedPageContextMenu')
			break
		case 'stop':
			await stopInstance('SyncedPageContextMenu')
			break
		case 'add_content':
			await router.push({
				path: `/browse/${instance.value?.loader === 'vanilla' ? 'datapack' : 'mod'}`,
				query: { i: route.params.id },
			})
			break
		case 'open_folder':
			if (instance.value) await showProfileInFolder(instance.value.path)
			break
		case 'copy_path': {
			if (instance.value) {
				const fullPath = await get_full_path(instance.value?.path)
				await navigator.clipboard.writeText(fullPath)
			}
			break
		}
	}
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
	const seconds = Math.floor(dur.asSeconds())
	return seconds + ' second' + (seconds > 1 ? 's' : '')
})

unlistenProfiles.value = await profile_listener(
	async (event: { profile_path_id: string; event: string }) => {
		if (event.profile_path_id !== route.params.id) return
		if (event.event === 'removed' || route.path === '/') {
			if (route.path !== '/') {
				await router.push({ path: '/' })
			}
			return
		}
		instance.value = await get(route.params.id as string).catch((err) => {
			if (String(err).includes('not managed')) {
				router.push({ path: '/' })
				return undefined
			}
			return handleError(err)
		})
		if (!instance.value?.linked_data?.project_id) {
			linkedProjectV3.value = undefined
		}
	},
)

if (_unmounted) {
	unlistenProfiles.value?.()
} else {
	unlistenProcesses.value = await process_listener(
		(e: { event: string; profile_path_id: string }) => {
			if (e.event === 'finished' && e.profile_path_id === route.params.id) {
				playing.value = false
			}
		},
	)
}
</script>
