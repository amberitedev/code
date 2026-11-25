import type { AbstractModrinthClient } from '@modrinth/api-client'
import {
	generatedIconSrcQueryOptions,
	type IconCacheContext,
	serverIconEditorConfig,
	serverIconImageQueryOptions,
	serverListQueryOptions,
} from '@modrinth/ui'
import type { QueryClient } from '@tanstack/vue-query'

/**
 * Warms the hosting caches for a signed-in user: their server list and the icons of their
 * available servers. Icons are skipped once `isCurrentSession` returns false.
 */
export async function prefetchHostingData(
	queryClient: QueryClient,
	client: AbstractModrinthClient,
	iconCache: IconCacheContext,
	isCurrentSession: () => boolean,
) {
	const response = await queryClient.fetchQuery(serverListQueryOptions(client)).catch(() => null)
	if (!response || !isCurrentSession()) return

	await Promise.allSettled(
		response.servers
			.filter((server) => server.status === 'available')
			.map(async (server) => {
				const metadata = server.icon
				if (!metadata) return
				const config = serverIconEditorConfig(metadata)
				if (config) {
					await queryClient.prefetchQuery(generatedIconSrcQueryOptions(config))
					return
				}
				const image = await queryClient.fetchQuery(
					serverIconImageQueryOptions(server.server_id, metadata.hash, client),
				)
				await iconCache.cacheIcon(image)
			}),
	)
}
