import { installs } from '@amberite/contracts/engine'
import type { ContractClient } from '@modrinth/api-client'
import { shareHistoryKey } from '@modrinth/ui'
import { useMutation, useQuery } from '@tanstack/vue-query'
import { computed, type MaybeRefOrGetter, toValue } from 'vue'

import { apiClient } from '@/platform/modrinth-client'

/**
 * Where a share's history is: an instance on this computer (`client.local`) or a server on a Core
 * (`client.core(id)`), each the engine's instance with the share's id.
 */
export type ShareHistorySource = { client: ContractClient; instanceId: string }

/** The source of a server's history on its Core, once the Core is known. */
export function useServerShareHistorySource(serverId: MaybeRefOrGetter<string | null | undefined>) {
	const core = useQuery({
		queryKey: computed(() => ['servers', 'core', toValue(serverId)]),
		queryFn: async () => (await apiClient.cores.coreOf(toValue(serverId)!)).id,
		enabled: computed(() => !!toValue(serverId)),
		staleTime: Infinity,
	})
	return computed<ShareHistorySource | null>(() => {
		const id = toValue(serverId)
		return id && core.data.value
			? { client: apiClient.core(core.data.value), instanceId: id }
			: null
	})
}

/**
 * A share's pushed versions on the host that holds them, newest first, over the engine's history
 * routes, which an App and a Core both serve. Restoring puts the instance back to a version, left
 * to push again; the latest version cannot be deleted.
 */
export function useShareHistory(source: MaybeRefOrGetter<ShareHistorySource | null>) {
	const call = () => {
		const from = toValue(source)
		if (!from) throw new Error('This history is not on this computer or a Core.')
		return from
	}
	const history = useQuery({
		queryKey: computed(() => shareHistoryKey(toValue(source)?.instanceId ?? '')),
		queryFn: () => {
			const { client, instanceId } = call()
			return client.call(installs.GetSharedInstanceHistory, { params: { id: instanceId } })
		},
		enabled: computed(() => !!toValue(source)),
		staleTime: 0,
	})
	const restore = useMutation({
		mutationFn: (version: number) => {
			const { client, instanceId } = call()
			return client.call(installs.RestoreSharedInstanceVersion, {
				params: { id: instanceId, version },
			})
		},
		onSettled: () => history.refetch(),
	})
	const remove = useMutation({
		mutationFn: (version: number) => {
			const { client, instanceId } = call()
			return client.call(installs.DeleteSharedInstanceVersion, {
				params: { id: instanceId, version },
			})
		},
		onSettled: () => history.refetch(),
	})
	return { history, restore, remove }
}
