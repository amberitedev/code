import type { Server } from '@amberite/contracts/core'
import type { QueryClient } from '@tanstack/vue-query'

import { serverKeys } from '#ui/composables/server-queries'

/** Forgets the server's own share once it is unpublished. */
export async function clearServerSharedInstance(
	queryClient: QueryClient,
	serverId: string,
	sharedInstanceId: string,
) {
	const detailKey = serverKeys.detail(serverId)
	const instanceKey = ['shared-instances', sharedInstanceId] as const
	const diffKey = serverKeys.shareDiff(serverId)
	await Promise.all([
		queryClient.cancelQueries({ queryKey: detailKey }),
		queryClient.cancelQueries({ queryKey: instanceKey }),
		queryClient.cancelQueries({ queryKey: diffKey }),
	])
	queryClient.setQueryData<Server>(detailKey, (server) =>
		server?.shared_instance_id === sharedInstanceId
			? { ...server, shared_instance_id: null }
			: server,
	)
	queryClient.removeQueries({ queryKey: instanceKey })
	queryClient.removeQueries({ queryKey: diffKey })
}
