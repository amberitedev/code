<script setup lang="ts">
import type { CoreInstanceSummary } from '@amberite/amberite-api'
import { PlusIcon } from '@modrinth/assets'
import {
	ButtonStyled,
	DropdownSelect,
	injectNotificationManager,
	NavTabs,
} from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { useStorage } from '@vueuse/core'
import { computed, inject, onMounted, onUnmounted, ref, watch, watchEffect } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { NewInstanceImage } from '@/assets/icons'
import GridDisplay from '@/components/GridDisplay.vue'
import { useCoreInstances } from '@/composables/useCoreInstances'
import { get_project_v3_many } from '@/helpers/cache.js'
import { profile_listener } from '@/helpers/events.js'
import { list, remove as removeProfile } from '@/helpers/profile.js'
import type { GameInstance } from '@/helpers/types'
import { useBreadcrumbs } from '@/store/breadcrumbs.js'

defineOptions({
	name: 'LibraryPage',
})

const { handleError } = injectNotificationManager()
const queryClient = useQueryClient()
const showCreationModal = inject('showCreationModal')
const route = useRoute()
const router = useRouter()
const breadcrumbs = useBreadcrumbs()
const instanceOpenAnimationDirection = useStorage(
	'app-library-instance-route-motion-direction',
	'Right',
)
const instanceOpenAnimationDirections = ['Right', 'Left', 'Up', 'Down', 'Middle']
const libraryContentAnimationMode = useStorage(
	'app-library-content-animation-mode',
	'Card changes',
)
const libraryContentAnimationModes = [
	'Card changes',
	'Subtle page',
	'In only',
	'Push',
	'None',
]

breadcrumbs.setRootContext({ name: 'Library', link: route.path })

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

async function cleanupLegacyServerProfiles(profiles: GameInstance[]): Promise<void> {
	for (const profile of profiles) {
		await removeProfile(profile.path).catch(handleError)
	}
	await queryClient.invalidateQueries({ queryKey: ['library', 'instances'] })
}

const initialPending = computed(
	() =>
		(instancesQuery.isPending.value || coreInstancesLoading.value) && instances.value.length === 0,
)
const hasLibraryContent = computed(() => instances.value.length > 0)
const libraryTabs = [
	{ label: 'All instances', href: '/library', kind: 'overview' },
	{ label: 'Modpacks', href: '/library/modpacks', kind: 'modpacks' },
	{ label: 'Servers', href: '/library/servers', kind: 'servers' },
	{ label: 'Custom', href: '/library/custom', kind: 'custom' },
	{ label: 'Shared with me', href: '/library/shared', kind: 'shared', shown: false },
	{ label: 'Saved', href: '/library/saved', kind: 'saved', shown: false },
]
const visibleLibraryTabs = computed(() => libraryTabs.filter((tab) => tab.shown ?? true))
function getLibraryTabKind(path: string) {
	if (path.endsWith('/modpacks')) return 'modpacks'
	if (path.endsWith('/servers')) return 'servers'
	if (path.endsWith('/custom')) return 'custom'
	return 'overview'
}

function isLibraryPath(path: string) {
	return path === '/library' || path.startsWith('/library/')
}

const activeLibraryTabKind = ref(getLibraryTabKind(route.path))
const libraryTabSlideDirection = ref<'forward' | 'backward'>('forward')
watch(
	() => route.path,
	(path) => {
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
	},
)
const activeLibraryTabIndex = computed(() =>
	visibleLibraryTabs.value.findIndex((tab) => tab.kind === activeLibraryTabKind.value),
)
const visibleLibraryTabIndex = activeLibraryTabIndex
const libraryRouteKey = computed(() => `library-tab:${activeLibraryTabKind.value}`)
const libraryContentAnimationModeValue = computed(() => {
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
		default:
			return instances.value
	}
})

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
window.addEventListener('offline', () => {
	offline.value = true
})
window.addEventListener('online', () => {
	offline.value = false
})

let unlistenProfile: (() => void) | undefined
onMounted(async () => {
	unlistenProfile = await profile_listener(async () => {
		await queryClient.invalidateQueries({ queryKey: ['library', 'instances'] })
	})
})
onUnmounted(() => {
	unlistenProfile?.()
})
</script>

<template>
	<div class="p-6 flex flex-col gap-3 relative min-h-full">
		<Teleport defer to="#sidebar-teleport-target">
			<div class="library-sidebar-controls">
				<div class="library-sidebar-control-section">
					<h3 class="text-base text-primary font-medium m-0">Library animation</h3>
					<DropdownSelect
						v-slot="{ selected }"
						v-model="instanceOpenAnimationDirection"
						name="Instance open animation direction"
						class="!w-full"
						:options="instanceOpenAnimationDirections"
					>
						<span class="font-semibold text-primary">Open: </span>
						<span class="font-semibold text-secondary">{{ selected }}</span>
					</DropdownSelect>
					<DropdownSelect
						v-slot="{ selected }"
						v-model="libraryContentAnimationMode"
						name="Library animation mode"
						class="!w-full"
						:options="libraryContentAnimationModes"
					>
						<span class="font-semibold text-primary">Animation: </span>
						<span class="font-semibold text-secondary">{{ selected }}</span>
					</DropdownSelect>
				</div>
			</div>
		</Teleport>
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
			:content-key="libraryRouteKey"
			:content-direction="libraryTabSlideDirection"
			:animation-mode="libraryContentAnimationModeValue"
		/>
		<div v-if="!initialPending && !hasLibraryContent" class="no-instance">
			<div class="icon">
				<NewInstanceImage />
			</div>
			<h3>No instances found</h3>
			<ButtonStyled color="brand">
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

.library-sidebar-controls {
	display: flex;
	flex-direction: column;
	gap: 1rem;
	padding: 1rem;
}

.library-sidebar-control-section {
	display: flex;
	flex-direction: column;
	gap: 0.75rem;
}
</style>
