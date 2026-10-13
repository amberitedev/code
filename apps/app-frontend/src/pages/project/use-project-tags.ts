import type { Labrinth } from '@modrinth/api-client'
import { injectNotificationManager } from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { shallowRef } from 'vue'

import { get_game_versions, get_loaders } from '@/helpers/tags'

// Tags do not change while the App runs. Browse fills the same query keys.
const tagQueryOptions = { staleTime: Infinity, gcTime: Infinity } as const

/**
 * Loader and game version tags for the project pages, shared through the query cache. It never
 * blocks setup: tags already in the cache are there for the first render, and missing ones fill in
 * when the read lands.
 */
export function useProjectTags() {
	const queryClient = useQueryClient()
	const { handleError } = injectNotificationManager()

	const loadersKey = ['tags', 'loaders']
	const gameVersionsKey = ['tags', 'game-versions']
	const loaders = shallowRef(queryClient.getQueryData<Labrinth.Tags.v2.Loader[]>(loadersKey) ?? [])
	const gameVersions = shallowRef(
		queryClient.getQueryData<Labrinth.Tags.v2.GameVersion[]>(gameVersionsKey) ?? [],
	)

	queryClient
		.ensureQueryData({
			queryKey: loadersKey,
			queryFn: () => get_loaders() as Promise<Labrinth.Tags.v2.Loader[]>,
			...tagQueryOptions,
		})
		.then((value) => {
			loaders.value = value
		})
		.catch(handleError)
	queryClient
		.ensureQueryData({
			queryKey: gameVersionsKey,
			queryFn: () => get_game_versions() as Promise<Labrinth.Tags.v2.GameVersion[]>,
			...tagQueryOptions,
		})
		.then((value) => {
			gameVersions.value = value
		})
		.catch(handleError)

	return { loaders, gameVersions }
}
