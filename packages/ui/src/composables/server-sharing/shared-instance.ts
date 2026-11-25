import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, type MaybeRefOrGetter, type Ref, toValue } from 'vue'

import type { InvitedPlayerRow } from '#ui/components/sharing/invited-players-table/types'
import { sharedInstanceKeys } from '#ui/composables/server-queries'
import { injectAuth, injectModrinthClient } from '#ui/providers'

import { useSharedInstanceEvent } from '../use-shared-instance-event'
import { remainingPlayerSlots } from './invite-link'

/**
 * Reads the query at `queryKey` again on each `shared_instance_users` event of `instanceId`. Every
 * mounted copy of the query hears the event; the first starts the read and the rest join it.
 */
function refetchOnUsersEvent(
	instanceId: Ref<string | null>,
	queryKey: MaybeRefOrGetter<readonly unknown[]>,
) {
	const queryClient = useQueryClient()
	useSharedInstanceEvent('shared_instance_users', instanceId, () => {
		void queryClient.invalidateQueries(
			{ queryKey: toValue(queryKey), exact: true },
			{ cancelRefetch: false },
		)
	})
}

export type ServerPlayerMembers = { rows: InvitedPlayerRow[]; remaining: number }

export function useServerPlayerMembers(serverId: string, instanceId: Ref<string | null>) {
	const client = injectModrinthClient()
	const auth = injectAuth()
	const userId = computed(() => auth.user.value?.id)
	const queryKey = computed(() =>
		sharedInstanceKeys.players(instanceId.value ?? '', serverId, userId.value),
	)
	const players = useQuery({
		queryKey,
		enabled: computed(() => !!instanceId.value && !!userId.value),
		queryFn: async (): Promise<ServerPlayerMembers> => {
			const response = await client.sharedinstances.players_v1.list(serverId, instanceId.value!)
			const users = response.users.length
				? await client.labrinth.users_v2.getMultiple(response.users.map((user) => user.id))
				: []
			const rows = response.users
				.filter((user) => user.join_type !== 'owner')
				.map((member): InvitedPlayerRow => {
					const user = users.find((user) => user.id === member.id)
					return {
						id: member.id,
						username: user?.username ?? member.id,
						avatarUrl: user?.avatar_url ?? undefined,
						joinedAt: member.joined_at ? new Date(member.joined_at) : null,
						lastPlayedAt: member.last_played ? new Date(member.last_played) : null,
						pending: !member.joined_at,
						method: member.join_type === 'link' ? 'link' : 'direct',
					}
				})
			return { rows, remaining: remainingPlayerSlots(response.users.length) }
		},
	})
	refetchOnUsersEvent(instanceId, queryKey)
	return players
}

export function useSharedInstanceInstalledOnce(instanceId: Ref<string | null>) {
	const client = injectModrinthClient()
	const auth = injectAuth()
	const userId = computed(() => auth.user.value?.id)
	const queryKey = computed(() => sharedInstanceKeys.instance(instanceId.value ?? '', userId.value))
	const instance = useQuery({
		queryKey,
		enabled: computed(() => !!instanceId.value && !!userId.value),
		queryFn: () => client.sharedinstances.instances_v1.get(instanceId.value!),
	})
	// The backend marks it installed when a player first joins, and tells the members then.
	refetchOnUsersEvent(instanceId, queryKey)
	return computed(() => instance.data.value?.installed_once ?? false)
}

/** A safe file name for a `.mrpack` named after `name`. */
export function mrpackFilename(name: string) {
	return `${name.replace(/[\\/:*?"<>|]/g, '_')}.mrpack`
}
