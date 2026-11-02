import { injectAuth, injectModrinthClient, serverKeys } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, type Ref } from 'vue'

import type { GameInstance } from '@/helpers/types'

/**
 * The server-first Core server a Play instance was made from, when the signed-in user owns it, so
 * its History tab can show the server's history read-only. `null` for any other instance, and
 * `undefined` while that is not known yet.
 *
 * A Play instance is a member of the server's own share (`shared_instance.role` is `member` and it
 * names its server, `server_manager_name`). The server is the one of the user's servers whose world
 * reports that share; a server that follows an instance reports the instance's share instead.
 */
export function usePlayInstanceServer(instance: Ref<GameInstance | undefined>) {
	const client = injectModrinthClient()
	const auth = injectAuth()
	const shareId = computed(() => {
		const shared = instance.value?.shared_instance
		return shared?.role === 'member' && shared.server_manager_name ? shared.id : null
	})
	const enabled = computed(
		() => client.selfHostedHosting && !!shareId.value && !!auth.user.value?.id,
	)
	const servers = useQuery({
		queryKey: ['servers', 'v1'],
		queryFn: () => client.archon.servers_v1.list(),
		enabled,
	})
	const match = computed(() => {
		for (const server of servers.data.value ?? []) {
			const world = server.worlds.find((each) => each.content?.shared_instance_id === shareId.value)
			if (world) return { serverId: server.id, worldId: world.id }
		}
		return null
	})
	const server = useQuery({
		queryKey: computed(() => serverKeys.detail(match.value?.serverId ?? '')),
		queryFn: () => client.archon.servers_v0.get(match.value!.serverId),
		enabled: computed(() => enabled.value && !!match.value),
	})
	return computed(() => {
		if (!enabled.value) return null
		if (!servers.data.value) return servers.isError.value ? null : undefined
		if (!match.value) return null
		if (!server.data.value) return server.isError.value ? null : undefined
		const owned =
			!server.data.value.linked_instance && server.data.value.owner_id === auth.user.value?.id
		return owned ? match.value : null
	})
}
