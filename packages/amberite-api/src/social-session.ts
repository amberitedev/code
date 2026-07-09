import type {
	AmberiteProfile,
	AmberiteUser,
	CoreListEntry,
	CorePresence,
	FriendGroupBan,
	FriendGroupMember,
	FriendGroupSummary,
} from './convex-types'
import type { FriendsListResult, GroupInviteWithGroup } from './convex-api'

export interface DurableSocialSessionState {
	currentUser: AmberiteUser | null
	profile?: AmberiteProfile | null
	friends: FriendsListResult | null
	coreLinks: CoreListEntry[]
	group?: FriendGroupSummary | null
	members?: FriendGroupMember[]
	bans?: FriendGroupBan[]
	pendingInvites?: GroupInviteWithGroup[]
	core?: CorePresence | null
}

export type LiveUserState = { online: boolean }

export interface LiveSocialState {
	users: Record<string, LiveUserState>
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
	live: LiveSocialState = { users: {} },
): SocialSessionState {
	const allowedUsers = new Set([
		durable.currentUser?.userId,
		...(durable.friends?.friends.map((friend) => friend.user?.userId) ?? []),
		...durable.coreLinks.flatMap((core) => core.memberUserIds ?? []),
		...(durable.members?.map((member) => member.userId) ?? []),
	].filter((id): id is string => !!id))
	return {
		...durable,
		live: {
			users: Object.fromEntries(Object.entries(live.users).filter(([userId]) => allowedUsers.has(userId))),
		},
	}
}
