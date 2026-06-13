<script setup lang="ts">
import type { CoreInstanceSummary } from '@amberite/amberite-api'
import { PlusIcon } from '@modrinth/assets'
import { ButtonStyled, injectNotificationManager, NavTabs, useLoadingBarToken } from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, inject, onMounted, onUnmounted, ref } from 'vue'
import { useRoute } from 'vue-router'

import { NewInstanceImage } from '@/assets/icons'
import AppPageSkeleton from '@/components/ui/AppPageSkeleton.vue'
import { useCoreInstances } from '@/composables/useCoreInstances'
import { profile_listener } from '@/helpers/events.js'
import { list } from '@/helpers/profile.js'
import type { GameInstance } from '@/helpers/types'
import { useBreadcrumbs } from '@/store/breadcrumbs.js'

defineOptions({
	name: 'LibraryPage',
})

const { handleError } = injectNotificationManager()
const queryClient = useQueryClient()
const showCreationModal = inject('showCreationModal')
const route = useRoute()
const breadcrumbs = useBreadcrumbs()

breadcrumbs.setRootContext({ name: 'Library', link: route.path })

const instancesQuery = useQuery({
	queryKey: ['library', 'instances'],
	queryFn: async () => (await list().catch(handleError)) ?? [],
	staleTime: 30_000,
	gcTime: 10 * 60_000,
})

const { instances: coreInstanceMap } = useCoreInstances()

function coreToGameInstance(inst: CoreInstanceSummary): GameInstance {
	return {
		path: inst.id,
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
	const appLibCoreIds = new Set(appLib.map((i) => i.core_instance_id ?? i.path))
	return [...appLib, ...coreServers.filter((i) => !appLibCoreIds.has(i.core_instance_id ?? i.path))]
})

const initialPending = computed(
	() => instancesQuery.isPending.value && instances.value.length === 0,
)
useLoadingBarToken(initialPending)

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
	<div class="p-6 flex flex-col gap-3">
		<AppPageSkeleton v-if="initialPending" variant="list" class="!p-0" />
		<h1 class="m-0 text-2xl hidden">Library</h1>
		<NavTabs
			:links="[
				{ label: 'All instances', href: `/library` },
				{ label: 'Modpacks', href: `/library/modpacks` },
				{ label: 'Servers', href: `/library/servers` },
				{ label: 'Custom', href: `/library/custom` },
				{ label: 'Shared with me', href: `/library/shared`, shown: false },
				{ label: 'Saved', href: `/library/saved`, shown: false },
			]"
		/>
		<template v-if="!initialPending && instances && instances.length > 0">
			<RouterView v-if="route.path.startsWith('/library')" :instances="instances" />
		</template>
		<div v-else-if="!initialPending" class="no-instance">
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
</style>
