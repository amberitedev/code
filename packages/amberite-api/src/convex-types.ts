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

export interface FriendGroupBan {
	_id: string
	friendGroupId: string
	userId: string
	bannedByUserId: string
	reason?: string
	createdAt: number
	user?: AmberiteUser | null
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

export interface ConvexProfileSnapshot {
	_id: string
	profileId: string
	authorUserId: string
	manifest: unknown
	clientOnlyManifest?: unknown
	serverManifest?: unknown
	notes?: string
	createdAt: number
}

export interface ConvexModSyncEvent {
	_id: string
	profileId: string
	snapshotId?: string
	authorUserId: string
	status: string
	message?: string
	createdAt: number
}

export type FriendGroupRoleName = 'owner' | 'admin' | 'member'
export type ProfileVisibility = 'everyone' | 'roles' | 'custom'
export type WhitelistScope = 'viewers' | 'roles' | 'custom'

export interface SyncedProfileSettings {
	name?: string
	syncEnabled?: boolean
	status?: 'active' | 'paused' | 'archived'
	visibility?: ProfileVisibility
	visibilityRoles?: FriendGroupRoleName[]
	visibilityUserIds?: string[]
	autoWhitelist?: boolean
	whitelistScope?: WhitelistScope
	whitelistRoles?: FriendGroupRoleName[]
	whitelistUserIds?: string[]
}

export interface ProfileWhitelistEntry {
	userId: string
	role: FriendGroupRoleName
	displayName?: string | null
	accounts: { gamertag: string | null; minecraftUuid: string | null }[]
}

export interface ProfileWhitelistResult {
	autoWhitelist: boolean
	entries: ProfileWhitelistEntry[]
}
