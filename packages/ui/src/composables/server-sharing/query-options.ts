import {
	type AbstractModrinthClient,
	ApiCallError,
	type ContractClient,
	instanceSharing,
} from '@modrinth/api-client'

import { serverKeys, sharedInstanceKeys } from '#ui/composables/server-queries'

import { sharedDiffItems } from './share-diff'

/** What pushing the server's content would change in its own share. 404 while it has none. */
export function serverShareDiffQueryOptions(
	core: ContractClient,
	serverId: string,
	running: boolean,
	userId: string | undefined,
) {
	return {
		queryKey: serverKeys.shareDiff(serverId, serverId, userId),
		queryFn: async () => {
			const preview = await core.call(instanceSharing.GetSharedInstancePublishPreview, {
				params: { id: serverId },
			})
			if (!preview) throw new ApiCallError(404, 'not_found', 'The server has no share.')
			const hasChanges = preview.diffs.length > 0
			const diff = {
				shared_instance_id: preview.sharedInstanceId,
				latest_version: preview.latestVersion,
				has_changes: hasChanges,
				// A running server takes server-side changes only once it restarts.
				requires_restart: hasChanges && running,
				diffs: preview.diffs,
			}
			return { diff, items: sharedDiffItems(diff.diffs) }
		},
		retry: false,
	}
}

/** The server's config files a push can carry. */
export function serverShareableFilesQueryOptions(
	core: ContractClient,
	serverId: string,
	userId: string | undefined,
) {
	return {
		queryKey: serverKeys.shareFiles(serverId, serverId, userId),
		queryFn: async () => {
			const preview = await core.call(instanceSharing.GetSharedInstancePublishPreview, {
				params: { id: serverId },
			})
			return { files: preview?.files ?? [], truncated: false }
		},
		retry: false,
	}
}

export function sharedInstanceInvitesQueryOptions(
	client: AbstractModrinthClient,
	serverId: string,
	instanceId: string,
	userId: string | undefined,
) {
	return {
		queryKey: sharedInstanceKeys.invites(instanceId, serverId, userId),
		queryFn: () => client.sharedinstances.players_v1.links(serverId, instanceId),
		retry: false,
	}
}
