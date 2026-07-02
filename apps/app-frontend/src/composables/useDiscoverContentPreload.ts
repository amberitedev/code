import type { Labrinth } from '@modrinth/api-client'
import type { ProjectType } from '@modrinth/ui'
import type { QueryClient } from '@tanstack/vue-query'

import { get_search_results_v3 } from '@/helpers/cache.js'
import { get_categories, get_game_versions, get_loaders } from '@/helpers/tags'

import { preloadQuery } from './useOptimisticPreload'

export const DISCOVER_PRELOAD_STALE_MS = 2 * 60_000
export const DISCOVER_METADATA_STALE_MS = 10 * 60_000
export const DISCOVER_PRELOAD_PROJECT_TYPES: ProjectType[] = [
	'modpack',
	'mod',
	'resourcepack',
	'datapack',
	'shader',
	'server',
]

export type RawDiscoverSearchResults = {
	result: Labrinth.Search.v3.SearchResults & {
		hits: (Labrinth.Search.v3.ResultSearchProject & { installed?: boolean })[]
	}
} | null

function formatDiscoverSearchFilterValue(value: string) {
	return value === 'true' || value === 'false' ? value : `\`${value}\``
}

function getDiscoverSearchProjectType(type: ProjectType) {
	return type === 'server' ? 'minecraft_java_server' : type
}

export function buildDiscoverSearchParams(type: ProjectType, limit = 20) {
	if (type === 'server') {
		return `?limit=${limit}&index=relevance&new_filters=${encodeURIComponent(
			'project_types = minecraft_java_server AND minecraft_java_server.ping.data EXISTS',
		)}`
	}

	return `?limit=${limit}&index=relevance&new_filters=${encodeURIComponent(
		`project_types = ${formatDiscoverSearchFilterValue(getDiscoverSearchProjectType(type))}`,
	)}`
}

export function getDiscoverProjectTypeFromHref(href: string): ProjectType | null {
	const match = href.match(/^\/browse\/([^?]+)/)
	const type = match?.[1] as ProjectType | undefined
	return type && DISCOVER_PRELOAD_PROJECT_TYPES.includes(type) ? type : null
}

export function preloadDiscoverSearchQuery(queryClient: QueryClient, type: ProjectType) {
	const requestParams = buildDiscoverSearchParams(type)
	return preloadQuery(queryClient, {
		queryKey: ['search', 'v3', requestParams],
		queryFn: () => get_search_results_v3(requestParams) as Promise<RawDiscoverSearchResults>,
		staleTime: DISCOVER_PRELOAD_STALE_MS,
	})
}

export function preloadDiscoverSearchQueries(
	queryClient: QueryClient,
	types: readonly ProjectType[] = DISCOVER_PRELOAD_PROJECT_TYPES,
) {
	for (const type of types) {
		void preloadDiscoverSearchQuery(queryClient, type)
	}
}

export function preloadDiscoverMetadataQueries(queryClient: QueryClient) {
	void preloadQuery(queryClient, {
		queryKey: ['tags', 'categories'],
		queryFn: get_categories,
		staleTime: DISCOVER_METADATA_STALE_MS,
	})
	void preloadQuery(queryClient, {
		queryKey: ['tags', 'loaders'],
		queryFn: get_loaders,
		staleTime: DISCOVER_METADATA_STALE_MS,
	})
	void preloadQuery(queryClient, {
		queryKey: ['tags', 'game-versions'],
		queryFn: get_game_versions,
		staleTime: DISCOVER_METADATA_STALE_MS,
	})
}

export function preloadDiscoverContentQueries(queryClient: QueryClient) {
	preloadDiscoverMetadataQueries(queryClient)
	preloadDiscoverSearchQueries(queryClient)
}
