<script setup lang="ts">
import {
	CoreOfflineError,
	installSyncedProfileFromCore,
	NetworkError,
	resolveInstallableSyncedProfiles,
	type CoreInstanceSummary,
	type InstallableSyncedProfile,
} from '@amberite/amberite-api'
import { PlusIcon } from '@modrinth/assets'
import { ButtonStyled, injectNotificationManager, NavTabs } from '@modrinth/ui'
import { join } from '@tauri-apps/api/path'
import { remove as removeFile, writeFile as writeFileBytes } from '@tauri-apps/plugin-fs'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { useStorage } from '@vueuse/core'
import { computed, inject, onMounted, onUnmounted, ref, watch, watchEffect } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { NewInstanceImage } from '@/assets/icons'
import GridDisplay from '@/components/GridDisplay.vue'
import { useCoreClient } from '@/composables/useCoreClient'
import { useCoreInstances } from '@/composables/useCoreInstances'
import { useSocial } from '@/composables/useSocial'
import { useSyncedServers } from '@/composables/useSyncedServers'
import { get_project_v3_many } from '@/helpers/cache.js'
import { profile_listener } from '@/helpers/events.js'
import { install_profile_from_file } from '@/helpers/pack'
import { create, edit, get_full_path, list, remove as removeProfile } from '@/helpers/profile.js'
import type { GameInstance } from '@/helpers/types'
import { getLinkedServerId, setLinkedServerId } from '@/pages/instance/synced/use-synced-link'
import { useBreadcrumbs } from '@/store/breadcrumbs.js'

defineOptions({
	name: 'LibraryPage',
})

const { addNotification, handleError } = injectNotificationManager()
const queryClient = useQueryClient()
const showCreationModal = inject('showCreationModal')
const route = useRoute()
const router = useRouter()
const breadcrumbs = useBreadcrumbs()
const core = useCoreClient()
const social = useSocial()
const syncedServers = useSyncedServers()
const libraryTabSwitchAnimationEnabled = false
const libraryContentAnimationMode = useStorage('app-library-content-animation-mode', 'Card changes')
const props = withDefaults(
	defineProps<{
		inertUnderlay?: boolean
		underlayPath?: string
	}>(),
	{
		inertUnderlay: false,
		underlayPath: '',
	},
)
const effectiveRoutePath = computed(() => props.underlayPath || route.path)

if (!props.inertUnderlay) {
	breadcrumbs.setRootContext({ name: 'Library', link: route.path })
}

const instancesQuery = useQuery({
	queryKey: ['library', 'instances'],
	queryFn: async () => {
		const profiles = (await list({ includeServerProfiles: true }).catch(handleError)) ?? []
		const legacyServerProfiles = profiles.filter((profile) => profile.profile_type === 'server')
		if (legacyServerProfiles.length > 0) {
			void cleanupLegacyServerProfiles(legacyServerProfiles)
		}
		return profiles.filter((profile) => profile.profile_type !== 'server')
	},
	staleTime: 30_000,
	gcTime: 10 * 60_000,
})

const coreSyncProfilesQuery = useQuery({
	queryKey: ['library', 'core-sync-profiles'],
	queryFn: () => core.listSyncProfiles(),
	enabled: computed(() => !props.inertUnderlay && !!social.group.value?.group.id),
	retry: false,
	staleTime: 30_000,
	gcTime: 10 * 60_000,
})

watch(
	() => social.group.value?.group.id,
	(friendGroupId) => {
		if (!friendGroupId || props.inertUnderlay) return
		void syncedServers.refresh(friendGroupId).catch(handleError)
	},
	{ immediate: true },
)

watch(
	() => syncedServers.error.value,
	(error) => {
		if (!error) return
		addNotification({
			title: 'Shared instances unavailable',
			text: error.message,
			type: 'warning',
		})
	},
)

const { instances: coreInstanceMap, loading: coreInstancesLoading } = useCoreInstances()

function coreToGameInstance(inst: CoreInstanceSummary): GameInstance {
	return {
		path: inst.path,
		install_stage: inst.install_status === 'ready' ? 'installed' : 'not_installed',
		profile_type: 'server',
		core_instance_id: inst.id,
		server_manifest_json: {
			name: inst.name,
			gameVersion: inst.game_version,
			modloader: inst.loader,
			loaderVersion: inst.loader_version,
			port: inst.port,
			memory: inst.memory,
		},
		name: inst.name,
		game_version: inst.game_version,
		loader: inst.loader as GameInstance['loader'],
		loader_version: inst.loader_version ?? undefined,
		groups: [],
		created: new Date(inst.created_at),
		modified: new Date(inst.updated_at),
		submitted_time_played: 0,
		recent_time_played: 0,
		hooks: {},
	} as GameInstance
}

const instances = computed(() => {
	const appLib: GameInstance[] = instancesQuery.data.value ?? []
	const coreServers = [...coreInstanceMap.value.values()].map(coreToGameInstance)
	return [...appLib, ...coreServers]
})

const localProfilesForSharedInstall = computed(() =>
	(instancesQuery.data.value ?? []).map((profile) =>
		profile.profile_type === 'synced'
			? {
					...profile,
					core_instance_id: profile.core_instance_id ?? getLinkedServerId(profile.path),
				}
			: profile,
	),
)

const installableInstances = computed(() => {
	if (!social.group.value || syncedServers.profiles.value.length === 0) return []
	const coreProfilesLoaded =
		coreSyncProfilesQuery.data.value !== undefined || coreSyncProfilesQuery.isError.value
	if (!coreProfilesLoaded) return []

	return resolveInstallableSyncedProfiles({
		socialProfiles: syncedServers.profiles.value,
		coreProfiles: coreSyncProfilesQuery.data.value ?? null,
		localProfiles: localProfilesForSharedInstall.value,
		coreAvailable: !coreSyncProfilesQuery.isError.value,
	})
})

async function cleanupLegacyServerProfiles(profiles: GameInstance[]): Promise<void> {
	for (const profile of profiles) {
		await removeProfile(profile.path).catch(handleError)
	}
	await queryClient.invalidateQueries({ queryKey: ['library', 'instances'] })
}

const libraryTabs = [
	{ label: 'All instances', href: '/library', kind: 'overview' },
	{ label: 'Modpacks', href: '/library/modpacks', kind: 'modpacks' },
	{ label: 'Servers', href: '/library/servers', kind: 'servers' },
	{ label: 'Custom', href: '/library/custom', kind: 'custom' },
	{ label: 'Shared', href: '/library/shared', kind: 'shared' },
	{ label: 'Saved', href: '/library/saved', kind: 'saved', shown: false },
]
const visibleLibraryTabs = computed(() => libraryTabs.filter((tab) => tab.shown ?? true))
function getLibraryTabKind(path: string) {
	if (path.endsWith('/modpacks')) return 'modpacks'
	if (path.endsWith('/servers')) return 'servers'
	if (path.endsWith('/custom')) return 'custom'
	if (path.endsWith('/shared')) return 'shared'
	return 'overview'
}

function isLibraryPath(path: string) {
	return path === '/library' || path.startsWith('/library/')
}

const activeLibraryTabKind = ref(getLibraryTabKind(effectiveRoutePath.value))
const libraryTabSlideDirection = ref<'forward' | 'backward'>('forward')
watch(effectiveRoutePath, (path) => {
	if (isLibraryPath(path)) {
		const previousIndex = visibleLibraryTabs.value.findIndex(
			(tab) => tab.kind === activeLibraryTabKind.value,
		)
		const nextKind = getLibraryTabKind(path)
		const nextIndex = visibleLibraryTabs.value.findIndex((tab) => tab.kind === nextKind)

		if (previousIndex >= 0 && nextIndex >= 0 && previousIndex !== nextIndex) {
			libraryTabSlideDirection.value = nextIndex > previousIndex ? 'forward' : 'backward'
		}

		activeLibraryTabKind.value = nextKind
	}
})
const activeLibraryTabIndex = computed(() =>
	visibleLibraryTabs.value.findIndex((tab) => tab.kind === activeLibraryTabKind.value),
)
const visibleLibraryTabIndex = activeLibraryTabIndex
const libraryRouteKey = computed(() => `library-tab:${activeLibraryTabKind.value}`)
const libraryContentAnimationModeValue = computed(() => {
	if (!libraryTabSwitchAnimationEnabled) return 'none'

	switch (libraryContentAnimationMode.value) {
		case 'Subtle page':
			return 'subtle-page'
		case 'In only':
			return 'in-only'
		case 'Push':
			return 'push'
		case 'None':
			return 'none'
		default:
			return 'card-changes'
	}
})

function selectLibraryTab(index: number, tab: { href: string }) {
	if (props.inertUnderlay) return

	if (index !== activeLibraryTabIndex.value) {
		libraryTabSlideDirection.value = index > activeLibraryTabIndex.value ? 'forward' : 'backward'
	}

	void router.push(tab.href).catch(() => undefined)
}

const serverProjectIds = ref(new Set<string>())
const linkedInstances = computed(() =>
	instances.value.filter((i) => i.profile_type !== 'server' && i.linked_data),
)
const typedServerInstances = computed(() =>
	instances.value.filter((i) => i.profile_type === 'server'),
)
const activeLibraryInstances = computed(() => {
	switch (activeLibraryTabKind.value) {
		case 'modpacks':
			return linkedInstances.value.filter((i) => {
				const projectId = i.linked_data?.project_id
				return !projectId || !serverProjectIds.value.has(projectId)
			})
		case 'servers':
			return [
				...typedServerInstances.value,
				...linkedInstances.value.filter((i) => {
					const projectId = i.linked_data?.project_id
					return !!projectId && serverProjectIds.value.has(projectId)
				}),
			]
		case 'custom':
			return instances.value.filter((i) => !i.linked_data)
		case 'shared':
			return []
		default:
			return instances.value
	}
})

const activeLibraryInstallableInstances = computed(() => {
	switch (activeLibraryTabKind.value) {
		case 'servers':
		case 'custom':
			return []
		case 'shared':
		case 'modpacks':
		default:
			return installableInstances.value
	}
})

const hasLibraryContent = computed(
	() =>
		activeLibraryInstances.value.length > 0 ||
		activeLibraryInstallableInstances.value.length > 0,
)
const initialPending = computed(
	() =>
		(instancesQuery.isPending.value ||
			coreInstancesLoading.value ||
			(!!social.group.value && syncedServers.loading.value) ||
			(!!social.group.value && coreSyncProfilesQuery.isPending.value)) &&
		!hasLibraryContent.value,
)

const installingSharedInstanceIds = ref(new Set<string>())
const installingInstallableIds = computed(() => [...installingSharedInstanceIds.value])

async function installSharedInstance(profile: InstallableSyncedProfile): Promise<void> {
	if (installingSharedInstanceIds.value.has(profile.coreInstanceId)) return

	if (profile.availability !== 'installable') {
		addNotification({
			title: 'Shared instance unavailable',
			text: profile.unavailableReason ?? 'This shared instance is not installable yet.',
			type: 'warning',
		})
		return
	}

	const nextInstalling = new Set(installingSharedInstanceIds.value)
	nextInstalling.add(profile.coreInstanceId)
	installingSharedInstanceIds.value = nextInstalling

	try {
		await installSyncedProfileFromCore<GameInstance>({
			core,
			profile,
			createProfile: ({ name, gameVersion, loader, profileType }) =>
				create(
					name,
					gameVersion,
					loader as GameInstance['loader'],
					null,
					null,
					true,
					null,
					profileType,
				),
			getProfileFullPath: get_full_path,
			joinPath: join,
			writeFile: writeFileBytes,
			removeFile,
			installMrpackFromPath: install_profile_from_file,
			editProfile: edit,
			removeProfile,
			linkServerId: setLinkedServerId,
		})
		await Promise.all([
			queryClient.invalidateQueries({ queryKey: ['library', 'instances'] }),
			queryClient.invalidateQueries({ queryKey: ['library', 'core-sync-profiles'] }),
		])
		addNotification({
			title: 'Instance installed',
			text: `${profile.name} is ready in your library.`,
			type: 'success',
		})
	} catch (error) {
		if (error instanceof CoreOfflineError || error instanceof NetworkError) {
			addNotification({
				title: 'Core unavailable',
				text: 'Connect to Core to install this shared instance.',
				type: 'warning',
			})
		} else {
			handleError(error instanceof Error ? error : new Error(String(error)))
		}
	} finally {
		const remainingInstalling = new Set(installingSharedInstanceIds.value)
		remainingInstalling.delete(profile.coreInstanceId)
		installingSharedInstanceIds.value = remainingInstalling
	}
}

watchEffect(async () => {
	const projectIds = [
		...new Set(
			linkedInstances.value
				.map((i) => i.linked_data?.project_id)
				.filter((projectId): projectId is string => typeof projectId === 'string'),
		),
	]
	if (projectIds.length === 0) {
		serverProjectIds.value = new Set()
		return
	}

	try {
		const projects = await get_project_v3_many(projectIds, 'must_revalidate')
		serverProjectIds.value = new Set(
			projects.filter((p) => p?.minecraft_server != null).map((p) => p.id),
		)
	} catch {
		serverProjectIds.value = new Set()
	}
})

const offline = ref(!navigator.onLine)

function handleOffline() {
	offline.value = true
}

function handleOnline() {
	offline.value = false
}

let unlistenProfile: (() => void) | undefined
onMounted(async () => {
	window.addEventListener('offline', handleOffline)
	window.addEventListener('online', handleOnline)
	unlistenProfile = await profile_listener(async () => {
		await queryClient.invalidateQueries({ queryKey: ['library', 'instances'] })
	})
})
onUnmounted(() => {
	window.removeEventListener('offline', handleOffline)
	window.removeEventListener('online', handleOnline)
	unlistenProfile?.()
})
</script>

<template>
	<div class="p-6 flex flex-col gap-3 relative min-h-full">
		<h1 class="m-0 text-2xl hidden">Library</h1>
		<div class="flex flex-wrap items-center justify-between gap-2">
			<NavTabs
				mode="local"
				:links="libraryTabs"
				:active-index="visibleLibraryTabIndex"
				@tab-click="selectLibraryTab"
			/>
		</div>
		<GridDisplay
			label="Instances"
			:instances="activeLibraryInstances"
			:installable-instances="activeLibraryInstallableInstances"
			:installing-installable-ids="installingInstallableIds"
			:content-key="libraryRouteKey"
			:content-direction="libraryTabSlideDirection"
			:animation-mode="libraryContentAnimationModeValue"
			@install-instance="installSharedInstance"
		/>
		<div v-if="!initialPending && !hasLibraryContent" class="no-instance">
			<div class="icon">
				<NewInstanceImage />
			</div>
			<h3>
				{{ activeLibraryTabKind === 'shared' ? 'No shared instances found' : 'No instances found' }}
			</h3>
			<ButtonStyled v-if="activeLibraryTabKind !== 'shared'" color="brand">
				<button :disabled="offline" @click="showCreationModal?.()">
					<PlusIcon />
					Create new instance
				</button>
			</ButtonStyled>
		</div>
	</div>
</template>

<style lang="scss" scoped>
.no-instance {
	display: flex;
	flex-direction: column;
	align-items: center;
	justify-content: center;
	height: 100%;
	gap: var(--gap-md);

	p,
	h3 {
		margin: 0;
	}

	.icon {
		svg {
			width: 10rem;
			height: 10rem;
		}
	}
}
</style>
