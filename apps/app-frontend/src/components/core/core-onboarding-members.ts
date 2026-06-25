import type { AmberiteUser, FriendGroupMember } from '@amberite/amberite-api'
import type { ServerAccessInviteSuggestion } from '@modrinth/ui'

import type { CoreAccessMember } from './core-access-types'

type FriendUser = AmberiteUser & { userId: string }

export const mockInviteUsers = [
	{ id: 'nora', userId: 'nora', username: 'Nora' },
	{ id: 'kai', userId: 'kai', username: 'Kai' },
	{ id: 'mika', userId: 'mika', username: 'Mika' },
	{ id: 'sol', userId: 'sol', username: 'Sol' },
]

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
