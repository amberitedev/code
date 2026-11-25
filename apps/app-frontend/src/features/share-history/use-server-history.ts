import type { Server } from '@amberite/contracts/core'
import { injectAuth } from '@modrinth/ui'
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
export function useServerHistory(server: Ref<Server | undefined>) {
	const auth = injectAuth()
	const linkedId = computed(() => server.value?.linked_instance?.id ?? null)
	const linked = useQuery({
		queryKey: computed(() => ['instances', linkedId.value, 'share-role'] as const),
		queryFn: async () => (await get(linkedId.value!).catch(() => null)) ?? null,
		enabled: computed(() => !!linkedId.value),
	})
	return computed(() => {
		const data = server.value
		if (!data || !auth.user.value?.id || data.owner_id !== auth.user.value.id) return null
		if (data.linked_instance) {
			if (linked.data.value?.shared_instance?.role === 'member') return null
			return { source: 'instance', instanceId: data.linked_instance.id } as const
		}
		if (!data.shared_instance_id) return null
		return { source: 'server', serverId: data.server_id } as const
	})
}
