/**
 * ConvexApiClient — typed wrapper for all Convex queries and mutations.
 * Reads the JWT from PlatformAdapter.getCurrentJwt() on each call.
 * Callers are responsible for ensuring a valid session before use.
 *
 * Key methods (line numbers): rawQuery:18, rawMutation:22, currentUser:28, setUsername:32,
 * ensureSocialProfile:40, searchUsers:44, friendsList:48, sendFriendRequest:57,
 * respondFriendRequest:65, removeFriend:69, blockUser:73, unblockUser:77,
 * listMyFriendGroups:82, getFriendGroup:86, listFriendGroupMembers:91,
 * ensureCoreFriendGroup, updateFriendGroup, updateMemberRole, removeMember,
 * friendGroupCores, createFriendGroupInvite, listMyGroupInvites, getInviteByCode,
 * acceptFriendGroupInvite, declineFriendGroupInvite, revokeFriendGroupInvite,
 * registerCore, corePresence, registerPairingCore, claimPairingCore.
 */
import type { PlatformAdapter } from './adapter'
import { convexQuery, convexMutation } from './convex-relay'
import type {
	AmberiteUser,
	CorePresence,
	FriendGroupInfo,
	FriendGroupSummary,
	FriendGroupMember,
	FriendGroupInvite,
} from './convex-types'

export class ConvexApiClient {
	constructor(public readonly adapter: PlatformAdapter) {}

	rawQuery<T = unknown>(path: string, args: unknown = {}): Promise<T> {
		return convexQuery<T>(this.adapter, path, args)
	}

	rawMutation<T = unknown>(path: string, args: unknown = {}): Promise<T> {
		return convexMutation<T>(this.adapter, path, args)
	}

	// ── Auth ──────────────────────────────────────────────────────────────────

	currentUser(): Promise<(AmberiteUser & { userId: string }) | null> {
		return convexQuery(this.adapter, 'auth:currentUser', {})
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

	ensureSocialProfile(): Promise<AmberiteUser> {
		return convexMutation(this.adapter, 'friends:ensureSocialProfile', {})
	}

	searchUsers(query: string): Promise<AmberiteUser[]> {
		return convexQuery(this.adapter, 'friends:searchUsers', { query })
	}

	friendsList(): Promise<{
		friends: { friendshipId: string; user: AmberiteUser | null; createdAt: number }[]
		incoming: unknown[]
		outgoing: unknown[]
		blocks: unknown[]
	}> {
		return convexQuery(this.adapter, 'friends:friendsList', {})
	}

	sendFriendRequest(args: {
		targetUserId?: string
		friendCode?: string
		username?: string
		message?: string
	}): Promise<{ requestId: string | null; status: string }> {
		return convexMutation(this.adapter, 'friends:sendFriendRequest', args)
	}

	respondFriendRequest(requestId: string, accept: boolean): Promise<null> {
		return convexMutation(this.adapter, 'friends:respondFriendRequest', { requestId, accept })
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

	claimPairingCore(code: string): Promise<{ coreId: string; connectionUrl?: string } | null> {
		return convexMutation(this.adapter, 'presence:claimPairingCore', { code })
	}
}
