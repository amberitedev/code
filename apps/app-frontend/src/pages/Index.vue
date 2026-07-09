<script setup lang="ts">
import { UserPlusIcon, UsersIcon } from '@modrinth/assets'
import { ButtonStyled, injectNotificationManager, useLoadingBarToken } from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import dayjs from 'dayjs'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute } from 'vue-router'

import RowDisplay from '@/components/RowDisplay.vue'
import RecentWorldsList from '@/components/ui/world/RecentWorldsList.vue'
import { get_search_results } from '@/helpers/cache.js'
import { profile_listener } from '@/helpers/events'
import { list } from '@/helpers/profile.js'
import { useBreadcrumbs } from '@/store/breadcrumbs'

defineOptions({
	name: 'HomePage',
})

const { handleError } = injectNotificationManager()
const queryClient = useQueryClient()
const route = useRoute()
const breadcrumbs = useBreadcrumbs()

breadcrumbs.setRootContext({ name: 'Home', link: route.path })

const instances = computed(() => instancesQuery.data.value ?? [])
const featuredModpacks = computed(() => featuredQuery.data.value?.modpacks ?? [])
const featuredMods = computed(() => featuredQuery.data.value?.mods ?? [])
const installedModpacksFilter = ref('')

const recentInstances = computed(() =>
	instances.value
		.filter((x) => x.last_played)
		.slice()
		.sort((a, b) => dayjs(b.last_played).diff(dayjs(a.last_played))),
)

const hasFeaturedProjects = computed(
	() => (featuredModpacks.value?.length ?? 0) + (featuredMods.value?.length ?? 0) > 0,
)

const offline = ref<boolean>(!navigator.onLine)
window.addEventListener('offline', () => {
	offline.value = true
})
window.addEventListener('online', () => {
	offline.value = false
})

async function fetchInstances() {
	const loadedInstances = (await list().catch(handleError)) ?? []

	const filters = []
	for (const instance of loadedInstances) {
		if (instance.linked_data && instance.linked_data.project_id) {
			filters.push(`NOT"project_id"="${instance.linked_data.project_id}"`)
		}
	}
	installedModpacksFilter.value = filters.join(' AND ')
	return loadedInstances
}

async function fetchFeaturedModpacks() {
	const response = await get_search_results(
		`?facets=[["project_type:modpack"]]&limit=10&index=follows&filters=${installedModpacksFilter.value}`,
	)

	if (response) {
		return response.result.hits
	}
	return []
}

async function fetchFeaturedMods() {
	const response = await get_search_results('?facets=[["project_type:mod"]]&limit=10&index=follows')

	if (response) {
		return response.result.hits
	}
	return []
}

async function refreshFeaturedProjects() {
	const [modpacks, mods] = await Promise.all([fetchFeaturedModpacks(), fetchFeaturedMods()])
	return { modpacks, mods }
}

const instancesQuery = useQuery({
	queryKey: ['app-home', 'instances'],
	queryFn: fetchInstances,
	staleTime: 30_000,
	gcTime: 10 * 60_000,
})

const featuredQuery = useQuery({
	queryKey: computed(() => ['app-home', 'featured', installedModpacksFilter.value]),
	queryFn: refreshFeaturedProjects,
	enabled: computed(() => !instancesQuery.isPending.value),
	staleTime: 5 * 60_000,
	gcTime: 15 * 60_000,
})

const initialPending = computed(
	() =>
		(instancesQuery.isPending.value && instances.value.length === 0) ||
		(featuredQuery.isPending.value && !hasFeaturedProjects.value),
)
useLoadingBarToken(initialPending)

let unlistenProfile: (() => void) | undefined

onMounted(async () => {
	unlistenProfile = await profile_listener(
		async (e: { event: string; profile_path_id: string }) => {
			await queryClient.invalidateQueries({ queryKey: ['app-home', 'instances'] })

			if (e.event === 'added' || e.event === 'created' || e.event === 'removed') {
				await instancesQuery.refetch()
				await queryClient.invalidateQueries({ queryKey: ['app-home', 'featured'] })
			}
		},
	)
})

onUnmounted(() => {
	unlistenProfile?.()
})
</script>

<template>
	<div class="p-6 flex flex-col gap-2">
		<h1 v-if="recentInstances?.length > 0" class="m-0 text-2xl font-extrabold">Welcome back!</h1>
		<h1 v-else class="m-0 text-2xl font-extrabold">Welcome to Modrinth App!</h1>
		<div class="flex flex-wrap gap-2">
			<ButtonStyled>
				<RouterLink to="/group/mock">
					<UsersIcon aria-hidden="true" />
					Mock group profile
				</RouterLink>
			</ButtonStyled>
			<ButtonStyled type="outlined">
				<RouterLink to="/group/mock-public">
					<UserPlusIcon aria-hidden="true" />
					Mock public group
				</RouterLink>
			</ButtonStyled>
		</div>
		<RecentWorldsList :recent-instances="recentInstances" />
		<RowDisplay
			v-if="hasFeaturedProjects"
			:instances="[
				{
					label: 'Discover a modpack',
					route: '/browse/modpack',
					instances: featuredModpacks,
					downloaded: false,
				},
				{
					label: 'Discover mods',
					route: '/browse/mod',
					instances: featuredMods,
					downloaded: false,
				},
			]"
			:can-paginate="true"
		/>
	</div>
</template>
