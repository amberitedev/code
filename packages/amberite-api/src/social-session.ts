import type {
	AmberiteUser,
	CorePresence,
	FriendGroupBan,
	FriendGroupMember,
	FriendGroupSummary,
} from './convex-types'
import type { FriendsListResult, GroupInviteWithGroup } from './convex-api'

export interface DurableSocialSessionState {
	currentUser: AmberiteUser | null
	friends: FriendsListResult | null
	group: FriendGroupSummary | null
	members: FriendGroupMember[]
	bans: FriendGroupBan[]
	pendingInvites: GroupInviteWithGroup[]
	core: CorePresence | null
}

export type LiveUserState = { online: boolean }
export type LiveCoreState = {
	online: boolean
	health?: 'healthy' | 'degraded' | 'offline'
	diagnostic?: 'none' | 'network' | 'authentication' | 'server'
}

export interface LiveSocialState {
	users: Record<string, LiveUserState>
	cores: Record<string, LiveCoreState>
}

export interface SocialSessionState extends DurableSocialSessionState {
	live: LiveSocialState
}

/**
 * Merges independently delivered durable and ephemeral state. Entries outside
 * the durable authorization set are discarded before UI code can render them.
 */
export function composeSocialSessionState(
	durable: DurableSocialSessionState,
	live: LiveSocialState = { users: {}, cores: {} },
): SocialSessionState {
	const allowedUsers = new Set([
		durable.currentUser?.userId,
		...(durable.friends?.friends.map((friend) => friend.user?.userId) ?? []),
		...durable.members.map((member) => member.userId),
	].filter((id): id is string => !!id))
	const allowedCores = new Set(durable.core?.coreId ? [durable.core.coreId] : [])
	return {
		...durable,
		live: {
			users: Object.fromEntries(Object.entries(live.users).filter(([userId]) => allowedUsers.has(userId))),
			cores: Object.fromEntries(Object.entries(live.cores).filter(([coreId]) => allowedCores.has(coreId))),
		},
	}
}
