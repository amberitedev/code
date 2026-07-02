import type { PlatformAdapter } from './adapter'
import { convexQuery, convexMutation, convexAction } from './convex-relay'
import type {
	AmberiteUser,
	AmberiteProfile,
	AmberitePublicProfile,
	ConvexModSyncEvent,
	ConvexProfileSnapshot,
	ConvexSyncedProfile,
	CoreListEntry,
	CorePresence,
	FriendGroupInfo,
	FriendGroupSummary,
	FriendGroupMember,
	FriendGroupInvite,
	FriendGroupBan,
	LinkedModrinthAccount,
	SyncedProfileSettings,
	ProfileWhitelistResult,
} from './convex-types'

export interface FriendRequestEntry {
	request: {
		_id: string
		fromUserId: string
		toUserId: string
		status: string
		createdAt: number
	}
	user: AmberiteUser | null
}

export interface FriendsListResult {
	friends: {
		friendshipId: string
		user: AmberiteUser | null
		presence?: { online: boolean; status?: string | null; lastSeenAt?: number | null }
		createdAt: number
	}[]
	incoming: FriendRequestEntry[]
	outgoing: FriendRequestEntry[]
	blocks: {
		blockId: string
		user: AmberiteUser | null
		createdAt: number
	}[]
}

export interface GroupInviteWithGroup {
	invite: FriendGroupInvite
	group: unknown
}

/**
 * Structural contract for the Amberite social/Core backend. `ConvexApiClient`
 * is the concrete implementation; pages and composables depend on this interface
 * so the backend can be swapped (e.g. a future native client) without churn.
 */
export interface AmberiteSocialClient {
	currentUser(): Promise<(AmberiteUser & { userId: string }) | null>
	currentProfile(): Promise<AmberiteProfile>
	getProfile(idOrUsername: string): Promise<AmberitePublicProfile | null>
	searchProfiles(query: string, limit?: number): Promise<AmberitePublicProfile[]>
	updateCurrentProfile(args: {
		displayName?: string
		username?: string
		bio?: string
		avatar?: null | {
			url: string
			storageId?: string
			mimeType?: string
			sizeBytes?: number
		}
	}): Promise<AmberiteProfile>
	linkedModrinthAccount(): Promise<LinkedModrinthAccount | null>
	storeModrinthOAuthTokens(args: {
		accessToken: string
		refreshToken?: string
		scopes: string[]
		expiresAt?: number
	}): Promise<LinkedModrinthAccount>
	refreshModrinthAccount(): Promise<LinkedModrinthAccount | null>
	disconnectModrinthAccount(): Promise<null>
	listMyCoreList(): Promise<CoreListEntry[]>
	listLinkedCoreList(): Promise<CoreListEntry[]>
	setUsername(username: string, displayName?: string): Promise<{ userId: string; username: string }>
	searchUsers(query: string): Promise<AmberiteUser[]>
	friendsList(): Promise<FriendsListResult>
	sendFriendRequest(args: {
		targetUserId: string
		message?: string
	}): Promise<{ requestId: string | null; status: string }>
	respondFriendRequest(requestId: string, accept: boolean): Promise<null>
	cancelFriendRequest(requestId: string): Promise<null>
	claimFriendRequestNotifications(): Promise<FriendRequestEntry[]>
	acknowledgeFriendRequestNotification(requestId: string): Promise<null>
	removeFriend(userId: string): Promise<null>
	blockUser(userId: string): Promise<null>
	unblockUser(userId: string): Promise<null>
	listMyFriendGroups(): Promise<FriendGroupSummary[]>
	getFriendGroup(
		friendGroupId: string,
	): Promise<{ group: FriendGroupInfo; core: CorePresence | null } | null>
	listFriendGroupMembers(friendGroupId: string): Promise<FriendGroupMember[]>
	ensureCoreFriendGroup(args: {
		coreId: string
		connectionUrl?: string
		name?: string
		description?: string
		banner?: string
		subdomain?: string
		setupMode?: 'remote' | 'local'
	}): Promise<{ friendGroupId: string }>
	updateFriendGroup(args: {
		friendGroupId: string
		name?: string
		description?: string
		banner?: string
		subdomain?: string
	}): Promise<null>
	updateMemberRole(args: {
		friendGroupId: string
		userId: string
		role: 'owner' | 'admin' | 'member'
		permissionPreset?: string
		customPermissions?: unknown
	}): Promise<null>
	removeMember(friendGroupId: string, userId: string): Promise<null>
	transferOwnership(friendGroupId: string, userId: string): Promise<null>
	leaveGroup(friendGroupId: string): Promise<null>
	banMember(friendGroupId: string, userId: string, reason?: string): Promise<null>
	unbanMember(friendGroupId: string, userId: string): Promise<null>
	listFriendGroupBans(friendGroupId: string): Promise<FriendGroupBan[]>
	friendGroupCores(friendGroupId: string): Promise<CorePresence[]>
	createFriendGroupInvite(args: {
		friendGroupId: string
		inviteeUserId?: string
		role?: 'owner' | 'admin' | 'member'
		ttlMs?: number
	}): Promise<{ inviteId: string; code?: string }>
	listMyGroupInvites(): Promise<GroupInviteWithGroup[]>
	getInviteByCode(code: string): Promise<GroupInviteWithGroup | null>
	acceptFriendGroupInvite(args: {
		inviteId?: string
		code?: string
	}): Promise<{ friendGroupId: string }>
	declineFriendGroupInvite(inviteId: string): Promise<null>
	revokeFriendGroupInvite(inviteId: string): Promise<null>
	registerCore(args: {
		coreId: string
		ownerUserId: string
		friendGroupId?: string
		connectionUrl?: string
		status?: string
		metadata?: unknown
	}): Promise<{ coreId: string }>
	corePresence(coreId: string): Promise<CorePresence | null>
	registerPairingCore(args: {
		code: string
		coreId: string
		connectionUrl?: string
		metadata?: unknown
		ttlMs?: number
	}): Promise<{ coreId: string; code: string }>
	claimPairingCore(
		code: string,
	): Promise<{ coreId: string; connectionUrl?: string; metadata?: unknown; realtimeCredential: string } | null>
	finalizePairingCore(args: {
		code: string
		coreId: string
		connectionUrl?: string
	}): Promise<{ coreId: string; friendGroupId: string }>
	releasePairingCore(args: { code: string; coreId: string }): Promise<null>
	registerSyncedProfile(args: {
		friendGroupId: string
		coreId: string
		coreInstanceId: string
		clientProfileId?: string
		name: string
		gameVersion?: string
		loader?: string
		syncEnabled?: boolean
	}): Promise<{ profileId: string }>
	listServerProfiles(friendGroupId: string): Promise<ConvexSyncedProfile[]>
	updateSyncedProfileSettings(
		profileId: string,
		settings: SyncedProfileSettings,
	): Promise<{ ok: boolean }>
	getProfileWhitelist(profileId: string): Promise<ProfileWhitelistResult>
	publishProfileSnapshot(args: {
		profileId: string
		manifest: unknown
		clientOnlyManifest?: unknown
		serverManifest?: unknown
		notes?: string
	}): Promise<{ snapshotId: string }>
	listProfileSnapshots(profileId: string): Promise<ConvexProfileSnapshot[]>
	listModSyncEvents(profileId: string): Promise<ConvexModSyncEvent[]>
}

export class ConvexApiClient implements AmberiteSocialClient {
	constructor(public readonly adapter: PlatformAdapter) {}

	rawQuery<T = unknown>(path: string, args: unknown = {}): Promise<T> {
		return convexQuery<T>(this.adapter, path, args)
	}

	rawMutation<T = unknown>(path: string, args: unknown = {}): Promise<T> {
		return convexMutation<T>(this.adapter, path, args)
	}

	rawAction<T = unknown>(path: string, args: unknown = {}, authenticate = true): Promise<T> {
		return convexAction<T>(this.adapter, path, args, authenticate)
	}

	// ── Auth ──────────────────────────────────────────────────────────────────

	currentUser(): Promise<(AmberiteUser & { userId: string }) | null> {
		return convexQuery(this.adapter, 'auth:currentUser', {})
	}

	currentProfile(): Promise<AmberiteProfile> {
		return convexQuery(this.adapter, 'profiles:current', {})
	}

	getProfile(idOrUsername: string): Promise<AmberitePublicProfile | null> {
		return convexQuery(this.adapter, 'profiles:get', { idOrUsername })
	}

	searchProfiles(query: string, limit?: number): Promise<AmberitePublicProfile[]> {
		return convexQuery(this.adapter, 'profiles:search', {
			query,
			...(limit !== undefined ? { limit } : {}),
		})
	}

	updateCurrentProfile(args: {
		displayName?: string
		username?: string
		bio?: string
		avatar?: null | {
			url: string
			storageId?: string
			mimeType?: string
			sizeBytes?: number
		}
	}): Promise<AmberiteProfile> {
		return convexMutation(this.adapter, 'profiles:updateCurrent', args)
	}

	linkedModrinthAccount(): Promise<LinkedModrinthAccount | null> {
		return convexQuery(this.adapter, 'modrinth:current', {})
	}

	storeModrinthOAuthTokens(args: {
		accessToken: string
		refreshToken?: string
		scopes: string[]
		expiresAt?: number
	}): Promise<LinkedModrinthAccount> {
		return convexAction(this.adapter, 'modrinth:storeCurrentOAuthTokens', args)
	}

	refreshModrinthAccount(): Promise<LinkedModrinthAccount | null> {
		return convexAction(this.adapter, 'modrinth:refreshCurrentStatus', {})
	}

	disconnectModrinthAccount(): Promise<null> {
		return convexMutation(this.adapter, 'modrinth:disconnectCurrent', {})
	}

	listMyCoreList(): Promise<CoreListEntry[]> {
		return convexQuery(this.adapter, 'coreList:listMine', {})
	}

	listLinkedCoreList(): Promise<CoreListEntry[]> {
		return convexQuery(this.adapter, 'coreList:listLinkedForCurrent', {})
	}

	setUsername(
		username: string,
		displayName?: string,
	): Promise<{ userId: string; username: string }> {
		return convexMutation(this.adapter, 'auth:setUsername', {
			username,
			...(displayName !== undefined ? { displayName } : {}),
		})
	}

	// ── Friends ───────────────────────────────────────────────────────────────

	searchUsers(query: string): Promise<AmberiteUser[]> {
		return convexQuery(this.adapter, 'friends:searchUsers', { query })
	}

	friendsList(): Promise<FriendsListResult> {
		return convexQuery(this.adapter, 'friends:friendsList', {})
	}

	sendFriendRequest(args: {
		targetUserId: string
		message?: string
	}): Promise<{ requestId: string | null; status: string }> {
		return convexMutation(this.adapter, 'friends:sendFriendRequest', args)
	}

	respondFriendRequest(requestId: string, accept: boolean): Promise<null> {
		return convexMutation(this.adapter, 'friends:respondFriendRequest', { requestId, accept })
	}

	cancelFriendRequest(requestId: string): Promise<null> {
		return convexMutation(this.adapter, 'friends:cancelFriendRequest', { requestId })
	}

	claimFriendRequestNotifications(): Promise<FriendRequestEntry[]> {
		return convexMutation(this.adapter, 'friends:claimFriendRequestNotifications', {})
	}

	acknowledgeFriendRequestNotification(requestId: string): Promise<null> {
		return convexMutation(this.adapter, 'friends:acknowledgeFriendRequestNotification', { requestId })
	}

	removeFriend(userId: string): Promise<null> {
		return convexMutation(this.adapter, 'friends:removeFriend', { userId })
	}

	blockUser(userId: string): Promise<null> {
		return convexMutation(this.adapter, 'friends:blockUser', { userId })
	}

	unblockUser(userId: string): Promise<null> {
		return convexMutation(this.adapter, 'friends:unblockUser', { userId })
	}

	// ── Friend Groups ─────────────────────────────────────────────────────────

	listMyFriendGroups(): Promise<FriendGroupSummary[]> {
		return convexQuery(this.adapter, 'friendGroups:listMyFriendGroups', {})
	}

	getFriendGroup(
		friendGroupId: string,
	): Promise<{ group: FriendGroupInfo; core: CorePresence | null } | null> {
		return convexQuery(this.adapter, 'friendGroups:getFriendGroup', { friendGroupId })
	}

	listFriendGroupMembers(friendGroupId: string): Promise<FriendGroupMember[]> {
		return convexQuery(this.adapter, 'friendGroups:listFriendGroupMembers', { friendGroupId })
	}

	ensureCoreFriendGroup(args: {
		coreId: string
		connectionUrl?: string
		name?: string
		description?: string
		banner?: string
		subdomain?: string
		setupMode?: 'remote' | 'local'
	}): Promise<{ friendGroupId: string }> {
		return convexMutation(this.adapter, 'friendGroups:ensureCoreFriendGroup', args)
	}

	updateFriendGroup(args: {
		friendGroupId: string
		name?: string
		description?: string
		banner?: string
		subdomain?: string
	}): Promise<null> {
		return convexMutation(this.adapter, 'friendGroups:updateFriendGroup', args)
	}

	updateMemberRole(args: {
		friendGroupId: string
		userId: string
		role: 'owner' | 'admin' | 'member'
		permissionPreset?: string
		customPermissions?: unknown
	}): Promise<null> {
		return convexMutation(this.adapter, 'friendGroups:updateMemberRole', args)
	}

	removeMember(friendGroupId: string, userId: string): Promise<null> {
		return convexMutation(this.adapter, 'friendGroups:removeMember', { friendGroupId, userId })
	}

	transferOwnership(friendGroupId: string, userId: string): Promise<null> {
		return convexMutation(this.adapter, 'friendGroups:transferOwnership', { friendGroupId, userId })
	}

	leaveGroup(friendGroupId: string): Promise<null> {
		return convexMutation(this.adapter, 'friendGroups:leaveGroup', { friendGroupId })
	}

	banMember(friendGroupId: string, userId: string, reason?: string): Promise<null> {
		return convexMutation(this.adapter, 'friendGroups:banMember', {
			friendGroupId,
			userId,
			...(reason !== undefined ? { reason } : {}),
		})
	}

	unbanMember(friendGroupId: string, userId: string): Promise<null> {
		return convexMutation(this.adapter, 'friendGroups:unbanMember', { friendGroupId, userId })
	}

	listFriendGroupBans(friendGroupId: string): Promise<FriendGroupBan[]> {
		return convexQuery(this.adapter, 'friendGroups:listFriendGroupBans', { friendGroupId })
	}

	friendGroupCores(friendGroupId: string): Promise<CorePresence[]> {
		return convexQuery(this.adapter, 'friendGroups:friendGroupCores', { friendGroupId })
	}

	// ── Group Invites ─────────────────────────────────────────────────────────

	createFriendGroupInvite(args: {
		friendGroupId: string
		inviteeUserId?: string
		role?: 'owner' | 'admin' | 'member'
		ttlMs?: number
	}): Promise<{ inviteId: string; code?: string }> {
		return convexMutation(this.adapter, 'groupInvites:createFriendGroupInvite', args)
	}

	listMyGroupInvites(): Promise<{ invite: FriendGroupInvite; group: unknown }[]> {
		return convexQuery(this.adapter, 'groupInvites:listMyGroupInvites', {})
	}

	getInviteByCode(code: string): Promise<{ invite: FriendGroupInvite; group: unknown } | null> {
		return convexQuery(this.adapter, 'groupInvites:getInviteByCode', { code })
	}

	acceptFriendGroupInvite(args: {
		inviteId?: string
		code?: string
	}): Promise<{ friendGroupId: string }> {
		return convexMutation(this.adapter, 'groupInvites:acceptFriendGroupInvite', args)
	}

	declineFriendGroupInvite(inviteId: string): Promise<null> {
		return convexMutation(this.adapter, 'groupInvites:declineFriendGroupInvite', { inviteId })
	}

	revokeFriendGroupInvite(inviteId: string): Promise<null> {
		return convexMutation(this.adapter, 'groupInvites:revokeFriendGroupInvite', { inviteId })
	}

	// ── Presence ──────────────────────────────────────────────────────────────

	registerCore(args: {
		coreId: string
		ownerUserId: string
		friendGroupId?: string
		connectionUrl?: string
		status?: string
		metadata?: unknown
	}): Promise<{ coreId: string }> {
		return convexMutation(this.adapter, 'presence:registerCore', args)
	}

	corePresence(coreId: string): Promise<CorePresence | null> {
		return convexQuery(this.adapter, 'presence:corePresence', { coreId })
	}

	registerPairingCore(args: {
		code: string
		coreId: string
		connectionUrl?: string
		metadata?: unknown
		ttlMs?: number
	}): Promise<{ coreId: string; code: string }> {
		return convexMutation(this.adapter, 'presence:registerPairingCore', args)
	}

	claimPairingCore(
		code: string,
	): Promise<{ coreId: string; connectionUrl?: string; metadata?: unknown; realtimeCredential: string } | null> {
		return convexMutation(this.adapter, 'presence:claimPairingCore', { code })
	}

	finalizePairingCore(args: {
		code: string
		coreId: string
		connectionUrl?: string
	}): Promise<{ coreId: string; friendGroupId: string }> {
		return convexMutation(this.adapter, 'presence:finalizePairingCore', args)
	}

	releasePairingCore(args: { code: string; coreId: string }): Promise<null> {
		return convexMutation(this.adapter, 'presence:releasePairingCore', args)
	}

	// ── Synced Profiles ───────────────────────────────────────────────────────

	registerSyncedProfile(args: {
		friendGroupId: string
		coreId: string
		coreInstanceId: string
		clientProfileId?: string
		name: string
		gameVersion?: string
		loader?: string
		syncEnabled?: boolean
	}): Promise<{ profileId: string }> {
		return convexMutation(this.adapter, 'sync:registerSyncedProfile', args)
	}

	listServerProfiles(friendGroupId: string): Promise<ConvexSyncedProfile[]> {
		return convexQuery(this.adapter, 'sync:listServerProfiles', { friendGroupId })
	}

	updateSyncedProfileSettings(
		profileId: string,
		settings: SyncedProfileSettings,
	): Promise<{ ok: boolean }> {
		return convexMutation(this.adapter, 'sync:updateSyncedProfileSettings', {
			profileId,
			...settings,
		})
	}

	getProfileWhitelist(profileId: string): Promise<ProfileWhitelistResult> {
		return convexQuery(this.adapter, 'sync:getProfileWhitelist', { profileId })
	}

	publishProfileSnapshot(args: {
		profileId: string
		manifest: unknown
		clientOnlyManifest?: unknown
		serverManifest?: unknown
		notes?: string
	}): Promise<{ snapshotId: string }> {
		return convexMutation(this.adapter, 'sync:publishProfileSnapshot', args)
	}

	listProfileSnapshots(profileId: string): Promise<ConvexProfileSnapshot[]> {
		return convexQuery(this.adapter, 'sync:listProfileSnapshots', { profileId })
	}

	listModSyncEvents(profileId: string): Promise<ConvexModSyncEvent[]> {
		return convexQuery(this.adapter, 'sync:listModSyncEvents', { profileId })
	}

}
