// TypeScript mirrors of Convex query/mutation return shapes.
// Derived from packages/convex/ function handlers — keep in sync with changes there.

export interface AmberiteUser {
	userId: string
	username?: string
	displayName?: string
	image?: string
	friendCode?: string
}

export interface CorePresence {
	coreId: string
	ownerUserId: string
	friendGroupId?: string
	connectionUrl?: string
	lastSeenAt: number
	status?: string
	metadata?: unknown
	name?: string
	subdomain?: string
	setupMode?: 'remote' | 'local'
}

export interface FriendGroupInfo {
	_id: string
	id: string
	name?: string
	description?: string
	banner?: string
	subdomain?: string
	coreId?: string
	ownerUserId: string
	createdAt: number
	updatedAt?: number
}

export interface FriendGroupSummary {
	group: FriendGroupInfo
	role: 'owner' | 'admin' | 'member'
	permissionPreset?: string
	core?: CorePresence | null
}

export interface FriendGroupMember {
	_id: string
	friendGroupId: string
	userId: string
	role: 'owner' | 'admin' | 'member'
	permissionPreset?: string
	customPermissions?: unknown
	createdAt: number
	updatedAt?: number
	user?: AmberiteUser | null
}

export interface FriendGroupInvite {
	_id: string
	friendGroupId: string
	inviterUserId: string
	inviteeUserId?: string
	code?: string
	role: 'owner' | 'admin' | 'member'
	status: 'pending' | 'accepted' | 'declined' | 'revoked' | 'expired'
	createdAt: number
	expiresAt: number
	respondedAt?: number
}

export interface ConvexSyncedProfile {
	_id: string
	friendGroupId: string
	coreId: string
	coreInstanceId: string
	clientProfileId?: string
	name: string
	gameVersion?: string
	loader?: string
	syncEnabled: boolean
	status: 'active' | 'paused' | 'archived'
	createdAt: number
	updatedAt: number
}
