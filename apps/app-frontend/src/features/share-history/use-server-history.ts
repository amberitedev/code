import { injectAuth, injectModrinthClient, serverKeys } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, type Ref } from 'vue'

import { get } from '@/helpers/instance'

/**
 * Where a Core server's History tab reads from, for the server's owner, or `null` when it has no
 * History tab.
 *
 * - `server`: a server-first server that is shared. Its Core holds the history; the tab restores
 *   and deletes.
 * - `instance`: a linked server (`linked_instance`). Its instance holds the history, in the App
 *   that pushes it; the tab shows it read-only. The instance may be on another of the owner's
 *   computers, and then the list is empty. Someone the instance is shared with may host it, and
 *   then it is not theirs to see.
 */
export function useServerHistory(serverId: Ref<string>) {
	const client = injectModrinthClient()
	const auth = injectAuth()
	const server = useQuery({
		queryKey: computed(() => serverKeys.detail(serverId.value)),
		queryFn: () => client.archon.servers_v0.get(serverId.value),
		enabled: computed(() => client.selfHostedHosting && !!serverId.value),
	})
	const serverFull = useQuery({
		queryKey: computed(() => serverKeys.detailV1(serverId.value)),
		queryFn: () => client.archon.servers_v1.get(serverId.value),
		enabled: computed(() => client.selfHostedHosting && !!serverId.value),
	})
	const linkedId = computed(() => server.data.value?.linked_instance?.id ?? null)
	const linked = useQuery({
		queryKey: computed(() => ['instances', linkedId.value, 'share-role'] as const),
		queryFn: async () => (await get(linkedId.value!).catch(() => null)) ?? null,
		enabled: computed(() => !!linkedId.value),
	})
	return computed(() => {
		const data = server.data.value
		if (!data || !auth.user.value?.id || data.owner_id !== auth.user.value.id) return null
		const world =
			serverFull.data.value?.worlds.find((each) => each.is_active) ??
			serverFull.data.value?.worlds[0]
		if (!world) return null
		if (data.linked_instance) {
			if (linked.data.value?.shared_instance?.role === 'member') return null
			return { source: 'instance', instanceId: data.linked_instance.id } as const
		}
		if (!world.content?.shared_instance_id) return null
		return { source: 'server', serverId: serverId.value, worldId: world.id } as const
	})
}
