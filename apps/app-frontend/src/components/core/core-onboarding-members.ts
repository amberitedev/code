import type { AmberiteUser, FriendGroupMember } from '@amberite/amberite-api'
import type { ServerAccessInviteSuggestion } from '@modrinth/ui'

import type { CoreAccessMember } from './core-access-types'

type FriendUser = AmberiteUser & { userId: string }

export function toAccessMember(member: FriendGroupMember): CoreAccessMember {
	return {
		id: member.userId,
		role: member.role === 'owner' ? 'owner' : member.role === 'admin' ? 'editor' : 'viewer',
		isOwner: member.role === 'owner',
		joinedAt: new Date(member.createdAt).toISOString(),
		user: {
			id: member.userId,
			username: member.user?.username || member.user?.displayName || member.userId,
			avatarUrl: member.user?.image,
		},
	}
}

export function toInviteSuggestion(user: AmberiteUser): ServerAccessInviteSuggestion {
	return {
		id: user.userId,
		username: user.username || user.displayName || user.userId,
		avatarUrl: user.image || user.avatar_url || undefined,
		email: user.email || undefined,
	}
}

export function toInviteCandidate(user: FriendUser): CoreAccessMember {
	return {
		id: `friend-${user.userId}`,
		user: { id: user.userId, username: user.username || user.displayName || user.userId, avatarUrl: user.image },
		role: 'viewer',
		joinedAt: null,
		inviteCandidate: true,
	}
}

export function toPendingInvite(user: ServerAccessInviteSuggestion): CoreAccessMember {
	return {
		id: user.id,
		user: { id: user.id, username: user.username, avatarUrl: user.avatarUrl },
		role: 'viewer',
		joinedAt: null,
		pending: true,
	}
}

export function matchesInviteSuggestion(user: ServerAccessInviteSuggestion, value: string) {
	return user.username.toLowerCase() === value || user.id.toLowerCase() === value || user.email?.toLowerCase() === value
}
