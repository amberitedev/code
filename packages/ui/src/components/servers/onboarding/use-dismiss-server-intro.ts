import type { Server } from '@modrinth/api-client'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { serverKeys } from '#ui/composables/server-queries'

/**
 * Ends a server's intro in the panel. A Core ends it itself once the server is installed, so this
 * only stops the panel showing it while the install runs.
 */
export function useDismissServerIntro() {
	const queryClient = useQueryClient()

	return useMutation({
		mutationFn: async (_serverId: string) => {},
		onSuccess: async (_result, serverId) => {
			const detailKey = serverKeys.detail(serverId)
			await queryClient.cancelQueries({ queryKey: detailKey })
			queryClient.setQueryData<Server>(detailKey, (server) =>
				server ? { ...server, flows: { ...server.flows, intro: false } } : server,
			)
		},
	})
}
