import type { SharedInstances } from '@modrinth/api-client'
import { injectModrinthClient, injectNotificationManager } from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { getErrorMessage } from '@/helpers/install'

export const sharedStorageKey = ['shared-storage'] as const
/** One share's kept versions, as its members see them. */
export const versionHistoryKey = (shareId: string) => ['shared-version-history', shareId] as const

/** The account's shared storage: its allowance, kept versions, and the shares it owns. */
export function useSharedStorage() {
	const client = injectModrinthClient()
	return useQuery({
		queryKey: sharedStorageKey,
		queryFn: () => client.sharedinstances.storage_v1.get(),
	})
}

/** Deletes an older version of one of the account's shares, then reloads its storage and history. */
export function useDeleteVersion() {
	const client = injectModrinthClient()
	const queryClient = useQueryClient()
	const { addNotification } = injectNotificationManager()
	return useMutation({
		mutationFn: ({ shareId, version }: { shareId: string; version: number }) =>
			client.sharedinstances.instances_v1.deleteVersion(shareId, version),
		onError: (cause) => {
			console.error('Failed to delete version', cause)
			addNotification({
				type: 'error',
				title: 'Could not delete the version',
				text: getErrorMessage(cause),
			})
		},
		onSettled: (_data, _error, { shareId }) =>
			Promise.all([
				queryClient.invalidateQueries({ queryKey: sharedStorageKey }),
				queryClient.invalidateQueries({ queryKey: versionHistoryKey(shareId) }),
			]),
	})
}

const SHARE_COLORS = ['bg-blue', 'bg-purple', 'bg-green', 'bg-orange', 'bg-red']

/**
 * The shares that use the allowance, largest first, each with the color it has in the storage
 * bar. The rest are only on the owner's own Cores.
 */
export function countedShares(storage: SharedInstances.Storage.v1.Storage) {
	return storage.shares
		.filter((share) => share.counted)
		.sort((a, b) => b.used - a.used)
		.map((share, index) => ({ ...share, color: SHARE_COLORS[index % SHARE_COLORS.length] }))
}
