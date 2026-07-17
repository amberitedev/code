// TypeScript mirrors of Convex query/mutation return shapes.
// Derived from packages/convex/ function handlers — keep in sync with changes there.

export interface AmberiteUser {
	id?: string
	userId: string
	username?: string
	minecraftUuid?: string
	verifiedMinecraftHandle?: string
	name?: string
	displayName?: string
	image?: string
	avatar_url?: string | null
	bio?: string | null
	created?: string
	friendCode?: string
	email?: string | null
	email_verified?: boolean
	auth_providers?: string[]
	has_password?: boolean
	has_totp?: boolean
	role?: string
	badges?: number
}

export interface AmberitePublicProfile {
	id: string
	userId: string
	username?: string
	minecraftUuid?: string
	verifiedMinecraftHandle?: string
	name?: string
	displayName?: string
	avatar_url: string | null
	image?: string
	bio: string | null
	created: string
	createdAt?: number | null
	profileUpdatedAt?: number | null
}

export interface AmberiteProfile extends AmberitePublicProfile {
	friendCode?: string
	avatarStorageId?: string
	avatarMimeType?: string
	avatarSizeBytes?: number
	deletedAt?: number
	deletedReason?: string
	email?: string | null
	email_verified?: boolean
	auth_providers?: string[]
	has_password?: boolean
	has_totp?: boolean
	role?: string
	badges?: number
}

export type ProfileSectionVisibility = 'everyone' | 'friends' | 'friend_group' | 'private'
export type ProfileRelationship =
	| 'self'
	| 'friend'
	| 'friend_group'
	| 'group_manager'
	| 'public'
	| 'blocked'

export interface ProfileVisibilitySettings {
	friends: ProfileSectionVisibility
	friendGroup: ProfileSectionVisibility
	corePresence: ProfileSectionVisibility
	favoriteModpacks: ProfileSectionVisibility
	achievements: ProfileSectionVisibility
}

export interface ProfileViewUserSummary extends AmberiteUser {
	id: string
	userId: string
	avatar_url: string | null
	bio?: string | null
	created?: string
}

export interface ProfileView {
	user: AmberitePublicProfile
	viewer: {
		relationship: ProfileRelationship
		isSelf: boolean
		isFriend: boolean
		isFriendGroup: boolean
		isGroupManager: boolean
		blocked: boolean
		viewerBlockedTarget: boolean
		targetBlockedViewer: boolean
	}
	settings?: ProfileVisibilitySettings
	favoriteModpacks?: {
		projectIds: string[]
	}
	achievements?: {
		achievementIds: string[]
	}
	friendGroup?: {
		group: {
			id: string
			_id: string
			name: string | null
			description: string | null
			banner: string | null
			subdomain: string | null
			coreId: string | null
			ownerUserId: string
			createdAt: number
			updatedAt: number | null
		}
		membership: {
			role: FriendGroupRoleName
			permissionPreset: string | null
			createdAt: number
			updatedAt: number | null
		}
	}
	corePresence?: {
		coreId: string
		ownerUserId: string
		linkState: 'unlinked' | 'linked' | null
		name: string | null
		subdomain: string | null
		setupMode: 'remote' | 'local' | null
		lastSeenAt: number | null
		status: string | null
	}
	friends?: {
		count: number
		hasMore: boolean
		items: ProfileViewUserSummary[]
		mutualCount: number
		mutualHasMore: boolean
		mutual: ProfileViewUserSummary[]
	}
	management?: {
		friendGroupId: string
		userId: string
		role?: FriendGroupRoleName
		permissionPreset?: string | null
		banned?: boolean
		canUpdateRole?: boolean
		canKick?: boolean
		canBan?: boolean
		canUnban?: boolean
		roles?: Array<'admin' | 'member'>
	}
	actions: {
		editProfile?: boolean
		editAccount?: boolean
		addFriend?: boolean
		cancelFriendRequest?: { requestId: string }
		acceptFriendRequest?: { requestId: string }
		declineFriendRequest?: { requestId: string }
		removeFriend?: boolean
		block?: boolean
		unblock?: boolean
		inviteToFriendGroup?: { friendGroupId: string }
		updateMemberRole?: { friendGroupId: string; roles: Array<'admin' | 'member'> }
		kick?: { friendGroupId: string }
		ban?: { friendGroupId: string }
		unban?: { friendGroupId: string }
	}
}

export interface LinkedModrinthAccount {
	id: string
	userId: string
	modrinthUserId: string
	username: string
	avatar_url: string | null
	scopes: string[]
	expiresAt: number | null
	status: 'active' | 'needs_reconnect' | 'revoked'
	needsReconnect: boolean
	reconnectReason?: string | null
	linkedAt: number
	updatedAt: number
}

export interface CoreListEntry {
	coreId: string
	ownerUserId: string
	linkState: 'unlinked' | 'linked'
	connectionUrl?: string
	setupMode?: 'remote' | 'local'
	createdAt: number
	lastSeenAt: number
	projectionRevision: number
	syncedAt: number
	isOwner: boolean
	memberUserIds?: string[]
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

export interface FriendGroupPublicProfile {
	group: FriendGroupInfo
	core: CorePresence | null
	members: FriendGroupMember[]
	viewer: {
		isMember: boolean
		role: FriendGroupRoleName | null
		permissionPreset?: string | null
		canManage: boolean
		isOwner: boolean
		banned: boolean
	}
	actions: {
		manage?: boolean
		leave?: boolean
		createInvite?: boolean
	}
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
