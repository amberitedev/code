import type { SharedInstances } from '@modrinth/api-client'
import { injectModrinthClient, injectNotificationManager } from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import {
	getErrorMessage,
	install_delete_shared_instance_version,
	install_get_shared_history_settings,
} from '@/helpers/install'

export const sharedStorageKey = ['shared-storage'] as const
/** The owner's history of one instance's share, kept on this computer. */
export const versionHistoryKey = (instanceId: string) =>
	['shared-version-history', instanceId] as const
export const sharedHistorySettingsKey = ['shared-history-settings'] as const

/** The account's shared storage: its allowance and the shares it owns. */
export function useSharedStorage() {
	const client = injectModrinthClient()
	return useQuery({
		queryKey: sharedStorageKey,
		queryFn: () => client.sharedinstances.storage_v1.get(),
	})
}

/** How many versions each share's local history keeps, and how many each has. */
export function useSharedHistorySettings() {
	return useQuery({
		queryKey: sharedHistorySettingsKey,
		queryFn: install_get_shared_history_settings,
	})
}

/** Deletes an older version from an instance's share history, then reloads the history. */
export function useDeleteVersion() {
	const queryClient = useQueryClient()
	const { addNotification } = injectNotificationManager()
	return useMutation({
		mutationFn: ({ instanceId, version }: { instanceId: string; version: number }) =>
			install_delete_shared_instance_version(instanceId, version),
		onError: (cause) => {
			console.error('Failed to delete version', cause)
			addNotification({
				type: 'error',
				title: 'Could not delete the version',
				text: getErrorMessage(cause),
			})
		},
		onSettled: (_data, _error, { instanceId }) =>
			Promise.all([
				queryClient.invalidateQueries({ queryKey: sharedHistorySettingsKey }),
				queryClient.invalidateQueries({ queryKey: versionHistoryKey(instanceId) }),
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
