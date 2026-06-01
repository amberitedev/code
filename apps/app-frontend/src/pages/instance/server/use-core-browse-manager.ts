import type { Labrinth } from '@modrinth/api-client'
import { CheckIcon, PlusIcon, SpinnerIcon } from '@modrinth/assets'
import type {
	BrowseInstallContext,
	BrowseSearchResponse,
	CardAction,
	FilterValue,
	Tags,
} from '@modrinth/ui'
import {
	commonMessages,
	defineMessages,
	injectNotificationManager,
	provideBrowseManager,
	useBrowseSearch,
	useVIntl,
} from '@modrinth/ui'
import { computed, type ComputedRef, ref } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'
import { get_search_results_v3 } from '@/helpers/cache.js'
import { get_categories, get_game_versions, get_loaders } from '@/helpers/tags'

import { injectCoreServerContext } from './core-server-instance'

const MOD_LOADERS = ['fabric', 'forge', 'quilt', 'neoforge']
const PLUGIN_LOADERS = ['paper', 'purpur']

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
})

export interface UseCoreBrowseManagerOptions {
	installedProjectIds: ComputedRef<Set<string>>
	onBack: () => void
	onInstalled: () => void | Promise<void>
}

/**
 * Assembles a Core-native browse manager for the shared `BrowsePageLayout`.
 *
 * Browsing reuses Modrinth's search engine (`useBrowseSearch` + the cached
 * `get_search_results_v3`), constrained to the Core instance's loader and game
 * version. Installation is delegated to Core's `add_mod_project`, which resolves
 * the latest compatible version and its dependencies. Client-only mods are
 * filtered from results (`environment = server`) and otherwise rejected by Core.
 */
export function useCoreBrowseManager(options: UseCoreBrowseManagerOptions) {
	const core = useCoreClient()
	const ctx = injectCoreServerContext()
	const { formatMessage } = useVIntl()
	const { addNotification, handleError } = injectNotificationManager()

	const categories = ref<Labrinth.Tags.v2.Category[]>([])
	const loaders = ref<Labrinth.Tags.v2.Loader[]>([])
	const gameVersions = ref<Labrinth.Tags.v2.GameVersion[]>([])

	get_categories()
		.then((v) => (categories.value = v ?? []))
		.catch(handleError)
	get_loaders()
		.then((v) => (loaders.value = v ?? []))
		.catch(handleError)
	get_game_versions()
		.then((v) => (gameVersions.value = v ?? []))
		.catch(handleError)

	const tags = computed<Tags>(() => ({
		gameVersions: gameVersions.value,
		loaders: loaders.value,
		categories: categories.value,
	}))

	const projectType = ref('mod')
	const installing = ref<Set<string>>(new Set())

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

	async function search(requestParams: string): Promise<BrowseSearchResponse> {
		const raw = (await get_search_results_v3(requestParams)) as {
			result: Labrinth.Search.v3.SearchResults & {
				hits: Labrinth.Search.v3.ResultSearchProject[]
			}
		} | null

		if (!raw) {
			return { projectHits: [], serverHits: [], total_hits: 0, per_page: 20 }
		}

		const projectHits = raw.result.hits.map(
			(hit) =>
				({
					...hit,
					project_id: hit.project_id ?? hit.id,
					title: hit.name ?? hit.title,
					description: hit.summary ?? hit.description,
					icon_url: hit.icon_url ?? null,
					project_type: hit.project_type ?? 'mod',
					slug: hit.slug ?? hit.project_id ?? hit.id,
				}) as unknown as Labrinth.Search.v2.ResultSearchProject,
		)

		return {
			projectHits,
			serverHits: [],
			total_hits: raw.result.total_hits ?? 0,
			per_page: raw.result.hits_per_page ?? 20,
		}
	}

	const searchState = useBrowseSearch({
		projectType,
		tags,
		providedFilters,
		search,
		persistentQueryParams: [],
	})

	const installContext = computed<BrowseInstallContext | null>(() => {
		const server = ctx.server.value
		if (!server) return null
		return {
			// Use the heading as the name so the sticky header shows "Add content"
			// instead of the server name (which is already in the page header above).
			name: formatMessage(messages.heading),
			loader: '',
			gameVersion: '',
			backUrl: '',
			backLabel: formatMessage(messages.back),
			heading: '',
			onBack: () => {
				options.onBack()
				return false
			},
		}
	})

	function getCardActions(
		result: Labrinth.Search.v2.ResultSearchProject | Labrinth.Search.v3.ResultSearchProject,
	): CardAction[] {
		const projectId = result.project_id
		const title = (result as Labrinth.Search.v2.ResultSearchProject).title ?? projectId
		const isInstalled = options.installedProjectIds.value.has(projectId)
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
						await core.addModProject(ctx.instanceId.value, projectId)
						addNotification({
							title: formatMessage(commonMessages.installedLabel),
							text: formatMessage(messages.added, { title }),
							type: 'success',
						})
						await options.onInstalled()
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
		variant: 'app',
		providedFilters,
		installContext,
		getCardActions,
		showProjectTypeTabs: computed(() => false),
		selectableProjectTypes: computed(() => []),
		getProjectLink: (result) => `/project/${result.project_id ?? result.slug}`,
		getServerProjectLink: (result) => `/project/${result.slug ?? result.project_id}`,
		lockedFilterMessages: {
			gameVersion: 'Provided by your server',
			modLoader: 'Provided by your server',
			environment: 'Provided by your server',
			providedBy: 'Provided by your server',
		},
	})

	void searchState.refreshSearch()

	return { searchState }
}
