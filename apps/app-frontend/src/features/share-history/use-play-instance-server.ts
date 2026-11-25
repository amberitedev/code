import { injectAuth, injectModrinthClient, serverListQueryOptions } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, type Ref } from 'vue'

import type { GameInstance } from '@/helpers/types'

/**
 * The server-first Core server a Play instance was made from, when the signed-in user owns it, so
 * its History tab can show the server's history read-only. `null` for any other instance, and
 * `undefined` while that is not known yet.
 *
 * A Play instance is a member of the server's own share (`shared_instance.role` is `member` and it
 * names its server, `server_manager_name`). The server is the one of the user's servers that reports
 * that share; a server that follows an instance reports the instance's share instead.
 */
export function usePlayInstanceServer(instance: Ref<GameInstance | undefined>) {
	const client = injectModrinthClient()
	const auth = injectAuth()
	const shareId = computed(() => {
		const shared = instance.value?.shared_instance
		return shared?.role === 'member' && shared.server_manager_name ? shared.id : null
	})
	const enabled = computed(() => !!shareId.value && !!auth.user.value?.id)
	const servers = useQuery({ ...serverListQueryOptions(client), enabled })
	return computed(() => {
		if (!enabled.value) return null
		const list = servers.data.value
		if (!list) return servers.isError.value ? null : undefined
		const server = list.servers.find((each) => each.shared_instance_id === shareId.value)
		const owned = !!server && !server.linked_instance && server.owner_id === auth.user.value?.id
		return owned ? { serverId: server.server_id } : null
	})
}
