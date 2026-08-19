import {
	injectNotificationManager,
	type InvitePlayersSearchUser,
	type InvitePlayersUser,
} from '@modrinth/ui'
import { computed, type Ref } from 'vue'

import { useSocial } from '@/composables/useSocial'

import { normalizeInviteKey, type ShareRow } from './shared-instance-share-types'

export function useSharedInstanceInviteCandidates(options: {
	rows: Ref<ShareRow[]>
	currentUserId: Ref<string | null>
	isSignedIn: Ref<boolean>
	actionsLocked: Ref<boolean>
}) {
	const { handleError } = injectNotificationManager()
	const social = useSocial()
	const invitedRows = computed(() => {
		const invited = new Map<string, ShareRow>()
		for (const row of options.rows.value) {
			invited.set(normalizeInviteKey(row.id), row)
			invited.set(normalizeInviteKey(row.username), row)
		}
		return invited
	})
	const inviteFriends = computed<InvitePlayersUser[]>(() =>
		(social.friends.value?.friends ?? [])
			.filter((friend) => friend.user)
			.sort((a, b) => Number(b.presence?.online) - Number(a.presence?.online))
			.map((friend) => {
				const user = friend.user!
				const id = user.userId
				const invited =
					invitedRows.value.get(normalizeInviteKey(id)) ??
					invitedRows.value.get(normalizeInviteKey(user.username ?? user.displayName ?? id))
				return {
					id,
					username: user.username ?? user.displayName ?? id,
					avatarUrl: user.image,
					online: friend.presence?.online ?? false,
					status: invited ? (invited.pending ? 'pending' : 'added') : 'available',
				}
			}),
	)
	const candidateKeys = computed(() => {
		const keys = new Set<string>()
		for (const friend of inviteFriends.value) {
			keys.add(normalizeInviteKey(friend.id))
			keys.add(normalizeInviteKey(friend.username))
		}
		return keys
	})

	async function search(query: string): Promise<InvitePlayersSearchUser[]> {
		if (options.actionsLocked.value) return []
		const ownUserId = options.currentUserId.value
		return (await social.searchUsers(query))
			.filter((user) => user.userId !== ownUserId)
			.filter((user) => {
				const id = normalizeInviteKey(user.userId)
				const username = normalizeInviteKey(user.username ?? user.displayName ?? user.userId)
				return (
					!candidateKeys.value.has(id) &&
					!candidateKeys.value.has(username) &&
					!invitedRows.value.has(id) &&
					!invitedRows.value.has(username)
				)
			})
			.map((user) => ({
				id: user.userId,
				username: user.username ?? user.displayName ?? user.userId,
				avatarUrl: user.image,
			}))
	}

	async function requestFriend(user: InvitePlayersUser) {
		if (options.actionsLocked.value) return
		const ownUserId = options.currentUserId.value
		if (ownUserId && normalizeInviteKey(user.id) === normalizeInviteKey(ownUserId)) return
		const isFriend = (social.friends.value?.friends ?? []).some(
			(friend) => friend.user?.userId === user.id,
		)
		if (!isFriend) await social.sendFriendRequest({ targetUserId: user.id }).catch(handleError)
	}

	return { inviteFriends, search, requestFriend }
}
