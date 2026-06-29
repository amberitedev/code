<template>
	<div class="flex flex-col gap-3 p-6">
		<BrowsePageLayout />
	</div>
</template>

<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { CheckIcon, PlusIcon, SpinnerIcon } from '@modrinth/assets'
import { computed, ref, type Ref } from 'vue'
import { useRouter } from 'vue-router'

import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { BrowsePageLayout } from '#ui/layouts/shared/browse-tab'
import type {
	BrowseInstallContext,
	BrowseSearchResponse,
	CardAction,
} from '#ui/layouts/shared/browse-tab/types'
import {
	provideBrowseManager,
	useBrowseSearch,
} from '#ui/layouts/shared/browse-tab'
import { injectHostingBackend, injectNotificationManager } from '#ui/providers'
import { commonMessages } from '#ui/utils/common-messages'
import type { FilterValue, Tags } from '#ui/utils/search'

import { injectCoreServerContext } from './context'

const MOD_LOADERS = ['fabric', 'forge', 'quilt', 'neoforge']
const PLUGIN_LOADERS = ['paper', 'purpur']
const MODRINTH_API_BASE = 'https://api.modrinth.com'

const props = withDefaults(
	defineProps<{
		variant?: 'app' | 'web'
		backPath?: string
	}>(),
	{
		variant: 'app',
		backPath: undefined,
	},
)

const messages = defineMessages({
	heading: {
		id: 'core-server.browse.heading',
		defaultMessage: 'Add content',
	},
	back: {
		id: 'core-server.browse.back',
		defaultMessage: 'Back to content',
	},
	installing: {
		id: 'core-server.browse.installing',
		defaultMessage: 'Installing...',
	},
	added: {
		id: 'core-server.browse.added',
		defaultMessage: '{title} was added to the server.',
	},
	gameVersionProvidedByServer: {
		id: 'core-server.browse.game-version-provided',
		defaultMessage: 'Game version is provided by the server',
	},
	modLoaderProvidedByServer: {
		id: 'core-server.browse.loader-provided',
		defaultMessage: 'Loader is provided by the server',
	},
	environmentProvidedByServer: {
		id: 'core-server.browse.environment-provided',
		defaultMessage: 'Environment is provided by the server',
	},
	providedByServer: {
		id: 'core-server.browse.provided-by',
		defaultMessage: 'Provided by the server',
	},
})

const router = useRouter()
const backend = injectHostingBackend()
const ctx = injectCoreServerContext()
const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()

const categories = ref<Labrinth.Tags.v2.Category[]>([])
const loaders = ref<Labrinth.Tags.v2.Loader[]>([])
const gameVersions = ref<Labrinth.Tags.v2.GameVersion[]>([])
const projectType = ref('mod')
const installing = ref<Set<string>>(new Set())
const installedProjectIds = ref<Set<string>>(new Set())

const tags = computed<Tags>(() => ({
	gameVersions: gameVersions.value,
	loaders: loaders.value,
	categories: categories.value,
})) as unknown as Ref<Tags>

const providedFilters = computed<FilterValue[]>(() => {
	const server = ctx.server.value
	if (!server) return []

	const filters: FilterValue[] = []
	if (server.mc_version) filters.push({ type: 'game_version', option: server.mc_version })

	const platform = server.loader?.toLowerCase()
	if (platform && MOD_LOADERS.includes(platform)) {
		filters.push({ type: 'mod_loader', option: platform })
	}
	if (platform && PLUGIN_LOADERS.includes(platform)) {
		filters.push({ type: 'plugin_loader', option: platform })
	}

	filters.push({ type: 'environment', option: 'server' })
	return filters
})

const installContext = computed<BrowseInstallContext | null>(() => {
	const server = ctx.server.value
	if (!server) return null

	return {
		name: formatMessage(messages.heading),
		loader: '',
		gameVersion: '',
		backUrl: props.backPath ?? `/hosting/manage/${encodeURIComponent(ctx.instanceId.value)}/content`,
		backLabel: formatMessage(messages.back),
		heading: '',
		onBack: () => {
			void router.push(
				props.backPath ?? `/hosting/manage/${encodeURIComponent(ctx.instanceId.value)}/content`,
			)
			return false
		},
	}
})

async function loadModrinth<T>(path: string): Promise<T> {
	const response = await fetch(`${MODRINTH_API_BASE}${path}`)
	if (!response.ok) {
		throw new Error(`Modrinth request failed: ${response.status} ${response.statusText}`)
	}
	return (await response.json()) as T
}

async function loadTags() {
	try {
		const [nextCategories, nextLoaders, nextGameVersions] = await Promise.all([
			loadModrinth<Labrinth.Tags.v2.Category[]>('/v2/tag/category'),
			loadModrinth<Labrinth.Tags.v2.Loader[]>('/v2/tag/loader'),
			loadModrinth<Labrinth.Tags.v2.GameVersion[]>('/v2/tag/game_version'),
		])
		categories.value = nextCategories
		loaders.value = nextLoaders
		gameVersions.value = nextGameVersions
	} catch (err) {
		handleError(err as Error)
	}
}

async function refreshInstalledProjectIds() {
	const mods = await backend.core.listMods(ctx.instanceId.value)
	installedProjectIds.value = new Set(
		mods
			.map((mod) => mod.modrinth_project_id)
			.filter((id): id is string => typeof id === 'string' && id.length > 0),
	)
}

async function search(requestParams: string): Promise<BrowseSearchResponse> {
	const suffix = requestParams.startsWith('?') ? requestParams : `?${requestParams}`
	const raw = await loadModrinth<
		| (Labrinth.Search.v3.SearchResults & {
				hits: Labrinth.Search.v3.ResultSearchProject[]
		  })
		| {
				result: Labrinth.Search.v3.SearchResults & {
					hits: Labrinth.Search.v3.ResultSearchProject[]
				}
		  }
	>(`/v3/search${suffix}`)
	const result = 'result' in raw ? raw.result : raw

	const projectHits = result.hits.map((hit) => {
		const projectId = hit.project_id ?? hit.id
		return {
			...hit,
			project_id: projectId,
			title: hit.name ?? hit.title,
			description: hit.summary ?? hit.description,
			icon_url: hit.icon_url ?? null,
			project_type: hit.project_type ?? 'mod',
			slug: hit.slug ?? projectId,
			installed: installedProjectIds.value.has(projectId),
		} as unknown as Labrinth.Search.v2.ResultSearchProject & {
			installed?: boolean
			installing?: boolean
		}
	})

	return {
		projectHits,
		serverHits: [],
		total_hits: result.total_hits ?? 0,
		per_page: result.hits_per_page ?? 20,
	}
}

const searchState = useBrowseSearch({
	projectType,
	tags,
	providedFilters,
	search,
	persistentQueryParams: [],
})

function getCardActions(
	result: Labrinth.Search.v2.ResultSearchProject | Labrinth.Search.v3.ResultSearchProject,
): CardAction[] {
	const projectId = result.project_id
	const title = (result as Labrinth.Search.v2.ResultSearchProject).title ?? projectId
	const isInstalled = installedProjectIds.value.has(projectId)
	const isInstalling = installing.value.has(projectId)

	return [
		{
			key: 'install',
			label: formatMessage(
				isInstalling
					? messages.installing
					: isInstalled
						? commonMessages.installedLabel
						: commonMessages.installButton,
			),
			icon: isInstalling ? SpinnerIcon : isInstalled ? CheckIcon : PlusIcon,
			iconClass: isInstalling ? 'animate-spin' : undefined,
			disabled: isInstalled || isInstalling,
			color: 'brand',
			type: 'outlined',
			onClick: async () => {
				if (isInstalled || isInstalling) return
				installing.value = new Set(installing.value).add(projectId)
				try {
					await backend.core.addModProject(ctx.instanceId.value, projectId)
					addNotification({
						title: formatMessage(commonMessages.installedLabel),
						text: formatMessage(messages.added, { title }),
						type: 'success',
					})
					await refreshInstalledProjectIds()
					await searchState.refreshSearch()
				} catch (err) {
					handleError(err as Error)
				} finally {
					const next = new Set(installing.value)
					next.delete(projectId)
					installing.value = next
				}
			},
		},
	]
}

provideBrowseManager({
	...searchState,
	tags,
	projectType,
	variant: props.variant,
	providedFilters,
	installContext,
	getCardActions,
	showProjectTypeTabs: computed(() => false),
	selectableProjectTypes: computed(() => []),
	getProjectLink: (result) => `/project/${result.project_id ?? result.slug}`,
	getServerProjectLink: (result) => `/project/${result.slug ?? result.project_id}`,
	lockedFilterMessages: {
		gameVersion: formatMessage(messages.gameVersionProvidedByServer),
		modLoader: formatMessage(messages.modLoaderProvidedByServer),
		environment: formatMessage(messages.environmentProvidedByServer),
		providedBy: formatMessage(messages.providedByServer),
	},
})

void loadTags().then(async () => {
	await refreshInstalledProjectIds()
	await searchState.refreshSearch()
})
</script>
