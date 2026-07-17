import { getAuthUserId } from '@convex-dev/auth/server'
import type { Id } from './_generated/dataModel'
import type { MutationCtx, QueryCtx } from './_generated/server'
import { normalizeMinecraftUuid } from './minecraftIdentity'
export type FriendGroupRole = 'owner' | 'admin' | 'member'
export type ProfileSectionVisibility = 'everyone' | 'friends' | 'friend_group' | 'private'
export type ProfileRelationshipKind =
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

export interface ProfileRelationshipContext {
	kind: ProfileRelationshipKind
	self: boolean
	friend: boolean
	friendGroup: boolean
	groupManager: boolean
	blocked: boolean
	viewerBlockedTarget: boolean
	targetBlockedViewer: boolean
	friendship: any | null
	outgoingRequest: any | null
	incomingRequest: any | null
	viewerMembership: any | null
	viewerManageableMembership: any | null
	targetMembership: any | null
	friendGroupDoc: any | null
	targetBan: any | null
}

export const DEFAULT_PROFILE_VISIBILITY: ProfileVisibilitySettings = {
	friends: 'friends',
	friendGroup: 'friend_group',
	corePresence: 'friend_group',
	favoriteModpacks: 'everyone',
	achievements: 'everyone',
}

export async function requireUserId(ctx: QueryCtx | MutationCtx): Promise<Id<'users'>> {
	const userId = await getAuthUserId(ctx)
	if (userId === null) throw new Error('not authenticated')
	return userId
}

/**
 * Resolve the acting user. In production this is always the authenticated user.
 *
 * When `AMBERITE_DEV_MODE` is enabled on the deployment, an explicit `devActAs`
 * user id is honoured so the desktop app and tests can act as any seeded user
 * before real Microsoft auth is wired. The flag is unset in production, so the
 * override is structurally impossible there — no auth bypass ships.
 */
export async function resolveActor(
	ctx: QueryCtx | MutationCtx,
	devActAs?: string,
): Promise<Id<'users'>> {
	if (devActAs && process.env.AMBERITE_DEV_MODE === 'true') {
		return devActAs as Id<'users'>
	}
	return await requireUserId(ctx)
}

/** True when the deployment is running with the dev identity override enabled. */
export function isDevMode(): boolean {
	return process.env.AMBERITE_DEV_MODE === 'true'
}

export function bansByGroup(ctx: QueryCtx | MutationCtx, friendGroupId: string) {
	return ctx.db
		.query('friendGroupBans')
		.withIndex('by_group', (q) => q.eq('friendGroupId', friendGroupId))
		.collect()
}

export function banByGroupUser(ctx: QueryCtx | MutationCtx, friendGroupId: string, userId: string) {
	return ctx.db
		.query('friendGroupBans')
		.withIndex('by_group_user', (q) => q.eq('friendGroupId', friendGroupId).eq('userId', userId))
		.unique()
}

/**
 * Rank roles so an actor can only act on members strictly below them. Owner
 * outranks admin outranks member. Used to gate kick/ban/role changes.
 */
export function roleRank(role: FriendGroupRole): number {
	return role === 'owner' ? 3 : role === 'admin' ? 2 : 1
}

export function profileVisibilitySettings(user: any): ProfileVisibilitySettings {
	return {
		...DEFAULT_PROFILE_VISIBILITY,
		...(user.profileVisibility ?? {}),
	}
}

export function normalizeProfileVisibilitySettings(
	patch: Partial<ProfileVisibilitySettings> | undefined,
	current?: Partial<ProfileVisibilitySettings>,
): ProfileVisibilitySettings {
	return {
		...DEFAULT_PROFILE_VISIBILITY,
		...(current ?? {}),
		...(patch ?? {}),
	}
}

export function canViewProfileSection(
	visibility: ProfileSectionVisibility,
	relationship: ProfileRelationshipContext,
): boolean {
	if (relationship.self) return true
	if (visibility === 'everyone') return true
	if (relationship.blocked) return false
	if (visibility === 'friends') return relationship.friend
	if (visibility === 'friend_group') return relationship.friendGroup || relationship.groupManager
	return false
}

export async function profileRelationship(
	ctx: QueryCtx | MutationCtx,
	viewerId: string,
	targetId: string,
): Promise<ProfileRelationshipContext> {
	const self = viewerId === targetId
	const [
		viewerBlockedTarget,
		targetBlockedViewer,
		friendship,
		outgoingRequest,
		incomingRequest,
		viewerMemberships,
		targetMemberships,
	] = await Promise.all([
		blockByPair(ctx, viewerId, targetId),
		blockByPair(ctx, targetId, viewerId),
		self ? null : acceptedFriendship(ctx, viewerId, targetId),
		self ? null : friendRequestByPair(ctx, viewerId, targetId),
		self ? null : friendRequestByPair(ctx, targetId, viewerId),
		membershipsByUser(ctx, viewerId),
		membershipsByUser(ctx, targetId),
	])
	const blocked = Boolean(viewerBlockedTarget || targetBlockedViewer)
	const viewerMembershipByGroup = new Map(
		viewerMemberships.map((membership) => [membership.friendGroupId, membership]),
	)
	const targetMembership =
		targetMemberships.find((membership) => viewerMembershipByGroup.has(membership.friendGroupId)) ??
		null
	const viewerMembership = targetMembership
		? (viewerMembershipByGroup.get(targetMembership.friendGroupId) ?? null)
		: null
	const friendGroupDoc = targetMembership
		? await ctx.db.get(targetMembership.friendGroupId as any)
		: null
	const viewerManageableMembership =
		viewerMemberships.find(
			(membership) => membership.role === 'owner' || membership.role === 'admin',
		) ?? null
	const targetBan = await manageableBanForTarget(ctx, viewerMemberships, targetId)
	const groupManager = Boolean(
		!blocked &&
		((viewerMembership &&
			targetMembership &&
			(viewerMembership.role === 'owner' || viewerMembership.role === 'admin') &&
			roleRank(targetMembership.role) < roleRank(viewerMembership.role)) ||
			targetBan),
	)
	const friendGroup = Boolean(!blocked && targetMembership)
	const friend = Boolean(!blocked && friendship)
	const kind: ProfileRelationshipKind = self
		? 'self'
		: blocked
			? 'blocked'
			: groupManager
				? 'group_manager'
				: friendGroup
					? 'friend_group'
					: friend
						? 'friend'
						: 'public'

	return {
		kind,
		self,
		friend,
		friendGroup,
		groupManager,
		blocked,
		viewerBlockedTarget: Boolean(viewerBlockedTarget),
		targetBlockedViewer: Boolean(targetBlockedViewer),
		friendship,
		outgoingRequest,
		incomingRequest,
		viewerMembership,
		viewerManageableMembership,
		targetMembership,
		friendGroupDoc,
		targetBan,
	}
}

async function manageableBanForTarget(
	ctx: QueryCtx | MutationCtx,
	viewerMemberships: any[],
	targetId: string,
) {
	for (const membership of viewerMemberships) {
		if (membership.role !== 'owner' && membership.role !== 'admin') continue
		const ban = await banByGroupUser(ctx, membership.friendGroupId, targetId)
		if (ban) return ban
	}
	return null
}

export async function requireFriendGroupRole(
	ctx: QueryCtx | MutationCtx,
	userId: string,
	friendGroupId: string,
	allowed: FriendGroupRole[],
) {
	const membership = await membershipByGroupUser(ctx, friendGroupId, userId)
	if (!membership || !allowed.includes(membership.role))
		throw new Error('not authorized for friend group')
}
export async function requireSingleGroupMembership(
	ctx: QueryCtx | MutationCtx,
	userId: string,
	allowedFriendGroupId?: string,
) {
	const memberships = await membershipsByUser(ctx, userId)
	const extra = memberships.find((membership) => membership.friendGroupId !== allowedFriendGroupId)
	if (extra) throw new Error('user is already in a friend group')
}
export async function requireSingleOwnedCore(
	ctx: QueryCtx | MutationCtx,
	ownerUserId: string,
	allowedCoreId?: string,
) {
	const cores = await coresByOwner(ctx, ownerUserId)
	const extra = cores.find((core) => core.coreId !== allowedCoreId)
	if (extra) throw new Error('user already owns a Core')
}
export async function ensureFriendGroupCore(
	ctx: MutationCtx,
	friendGroupId: string,
	coreId: string,
	now: number,
) {
	const group = await ctx.db.get(friendGroupId as Id<'friendGroups'>)
	if (!group) throw new Error('friend group not found')
	if (group.coreId && group.coreId !== coreId) throw new Error('friend group already has a Core')
	if (!group.coreId) await ctx.db.patch(group._id, { coreId, updatedAt: now })
	return group
}
export async function getOrCreateDefaultFriendGroup(
	ctx: MutationCtx,
	userId: string,
	coreId: string,
	now: number,
) {
	const group = await groupByCoreId(ctx, coreId)
	if (group) {
		const groupId = group._id.toString()
		await requireSingleGroupMembership(ctx, userId, groupId)
		await requireSingleOwnedCore(ctx, userId, coreId)
		await requireFriendGroupRole(ctx, userId, groupId, ['owner'])
		return groupId
	}

	const core = await coreById(ctx, coreId)
	if (core && core.ownerUserId !== userId) throw new Error('Core already belongs to another user')
	if (core?.friendGroupId) {
		await ensureFriendGroupCore(ctx, core.friendGroupId, coreId, now)
		await requireSingleGroupMembership(ctx, userId, core.friendGroupId)
		await requireSingleOwnedCore(ctx, userId, coreId)
		await requireFriendGroupRole(ctx, userId, core.friendGroupId, ['owner'])
		return core.friendGroupId
	}

	await requireSingleGroupMembership(ctx, userId)
	await requireSingleOwnedCore(ctx, userId, coreId)
	const friendGroupId = await ctx.db.insert('friendGroups', {
		ownerUserId: userId,
		coreId,
		createdAt: now,
		updatedAt: now,
	})
	const id = friendGroupId.toString()
	await ctx.db.insert('friendGroupMembers', {
		friendGroupId: id,
		userId,
		role: 'owner',
		permissionPreset: 'owner',
		createdAt: now,
		updatedAt: now,
	})
	return id
}
export async function assertCoreCanBelongToGroup(
	ctx: QueryCtx | MutationCtx,
	coreId: string,
	ownerUserId: string,
	friendGroupId: string,
) {
	const core = await coreById(ctx, coreId)
	if (core && core.ownerUserId !== ownerUserId)
		throw new Error('Core already belongs to another user')
	if (core?.friendGroupId && core.friendGroupId !== friendGroupId)
		throw new Error('Core already belongs to another friend group')
	await requireSingleOwnedCore(ctx, ownerUserId, coreId)
	return core
}
export async function upsertCoreForFriendGroup(
	ctx: MutationCtx,
	coreId: string,
	ownerUserId: string,
	friendGroupId: string,
	args: {
		name?: string
		subdomain?: string
		setupMode?: 'remote' | 'local'
		connectionUrl?: string
		metadata?: unknown
	},
	now: number,
) {
	const existingCore = await assertCoreCanBelongToGroup(ctx, coreId, ownerUserId, friendGroupId)
	const value = {
		coreId,
		ownerUserId,
		friendGroupId,
		...(args.name !== undefined ? { name: args.name } : {}),
		...(args.subdomain !== undefined ? { subdomain: args.subdomain } : {}),
		...(args.setupMode !== undefined ? { setupMode: args.setupMode } : {}),
		...(args.connectionUrl !== undefined ? { connectionUrl: args.connectionUrl } : {}),
		lastSeenAt: now,
		status: 'paired',
		metadata: args.metadata,
	}
	if (existingCore) await ctx.db.patch(existingCore._id, value)
	else await ctx.db.insert('cores', value)
}
export interface CurrentAccountFields {
	minecraftUuid: string | null
	verifiedMinecraftHandle: string | null
	email: string | null
	email_verified: boolean
	auth_providers: string[]
	has_password: boolean
	has_totp: boolean
	role: string
	badges: number
}
export async function currentAccountFields(
	ctx: QueryCtx | MutationCtx,
	userId: Id<'users'>,
): Promise<CurrentAccountFields> {
	const user = await ctx.db.get(userId)
	const accounts = await ctx.db
		.query('authAccounts')
		.withIndex('userIdAndProvider', (q) => q.eq('userId', userId))
		.collect()
	const linkedMinecraft = user?.amberiteUserId
		? await ctx.db
				.query('linkedMicrosoftAccounts')
				.withIndex('by_amberite_user', (q) => q.eq('amberiteUserId', user.amberiteUserId!))
				.first()
		: null
	const minecraftAccount = accounts.find((account) => account.provider === 'minecraft-token')
	return {
		minecraftUuid:
			user?.minecraftUuid ??
			linkedMinecraft?.minecraftUuid ??
			minecraftUuidFromProviderAccount(minecraftAccount?.providerAccountId) ??
			null,
		verifiedMinecraftHandle: user?.verifiedMinecraftHandle ?? linkedMinecraft?.gamertag ?? null,
		email: user?.email ?? null,
		email_verified: Boolean(user?.emailVerificationTime),
		auth_providers: minecraftAccount ? ['minecraft'] : [],
		has_password: false,
		has_totp: false,
		role: '',
		badges: 0,
	}
}

function minecraftUuidFromProviderAccount(providerAccountId: string | undefined): string | null {
	if (!providerAccountId?.startsWith('minecraft:')) return null
	try {
		return normalizeMinecraftUuid(providerAccountId.slice('minecraft:'.length))
	} catch {
		return null
	}
}

export function publicUser(
	user: any,
	includeFriendCode = false,
	accountFields?: CurrentAccountFields,
) {
	const avatarUrl = user.avatarUrl ?? user.image
	return {
		id: user._id,
		userId: user._id,
		username: accountFields?.verifiedMinecraftHandle ?? user.verifiedMinecraftHandle,
		minecraftUuid: accountFields?.minecraftUuid ?? user.minecraftUuid ?? null,
		verifiedMinecraftHandle: accountFields?.verifiedMinecraftHandle ?? user.verifiedMinecraftHandle,
		displayName: user.displayName,
		name:
			user.displayName ?? accountFields?.verifiedMinecraftHandle ?? user.verifiedMinecraftHandle,
		image: user.image,
		avatar_url: avatarUrl ?? null,
		bio: user.bio ?? null,
		created: new Date(user._creationTime ?? Date.now()).toISOString(),
		...(accountFields ?? {}),
		...(includeFriendCode && user.friendCode ? { friendCode: user.friendCode } : {}),
	}
}
export function publicProfile(user: any) {
	const avatarUrl = user.avatarUrl ?? user.image
	return {
		id: user._id,
		userId: user._id,
		username: user.verifiedMinecraftHandle,
		minecraftUuid: user.minecraftUuid ?? null,
		verifiedMinecraftHandle: user.verifiedMinecraftHandle,
		name: user.displayName ?? user.verifiedMinecraftHandle,
		displayName: user.displayName,
		avatar_url: avatarUrl ?? null,
		image: user.image,
		bio: user.bio ?? null,
		created: new Date(user._creationTime ?? Date.now()).toISOString(),
		createdAt: user._creationTime ?? null,
		profileUpdatedAt: user.profileUpdatedAt ?? null,
	}
}
export function publicCurrentProfile(user: any, accountFields?: CurrentAccountFields) {
	return {
		...publicProfile(user),
		friendCode: user.friendCode,
		avatarStorageId: user.avatarStorageId,
		avatarMimeType: user.avatarMimeType,
		avatarSizeBytes: user.avatarSizeBytes,
		deletedAt: user.deletedAt,
		deletedReason: user.deletedReason,
		...(accountFields ?? {}),
	}
}
export function publicLegacyCore(core: any) {
	if (!core) return null
	return {
		_id: core._id,
		coreId: core.coreId,
		ownerUserId: core.ownerUserId,
		friendGroupId: core.friendGroupId,
		name: core.name,
		subdomain: core.subdomain,
		setupMode: core.setupMode,
		connectionUrl: core.connectionUrl,
		lastSeenAt: core.lastSeenAt,
		status: core.status,
		metadata: core.metadata,
	}
}
export function coreById(ctx: QueryCtx | MutationCtx, coreId: string) {
	return ctx.db
		.query('cores')
		.withIndex('by_core_id', (q) => q.eq('coreId', coreId))
		.unique()
}
export function groupByCoreId(ctx: QueryCtx | MutationCtx, coreId: string) {
	return ctx.db
		.query('friendGroups')
		.withIndex('by_core_id', (q) => q.eq('coreId', coreId))
		.unique()
}
export async function groupByIdOrSubdomain(ctx: QueryCtx | MutationCtx, idOrSubdomain: string) {
	const normalized = idOrSubdomain.trim().toLowerCase()
	if (!normalized) return null

	const groupId = ctx.db.normalizeId('friendGroups', idOrSubdomain)
	if (groupId) {
		const groupById = await ctx.db.get(groupId)
		if (groupById) return groupById
	}

	return await ctx.db
		.query('friendGroups')
		.withIndex('by_subdomain', (q) => q.eq('subdomain', normalized))
		.first()
}
export function membershipsByUser(ctx: QueryCtx | MutationCtx, userId: string) {
	return ctx.db
		.query('friendGroupMembers')
		.withIndex('by_user', (q) => q.eq('userId', userId))
		.collect()
}
export function membersByGroup(ctx: QueryCtx, friendGroupId: string) {
	return ctx.db
		.query('friendGroupMembers')
		.withIndex('by_group', (q) => q.eq('friendGroupId', friendGroupId))
		.collect()
}
export function membershipByGroupUser(
	ctx: QueryCtx | MutationCtx,
	friendGroupId: string,
	userId: string,
) {
	return ctx.db
		.query('friendGroupMembers')
		.withIndex('by_group_user', (q) => q.eq('friendGroupId', friendGroupId).eq('userId', userId))
		.unique()
}
export function coresByOwner(ctx: QueryCtx | MutationCtx, ownerUserId: string) {
	return ctx.db
		.query('cores')
		.withIndex('by_owner', (q) => q.eq('ownerUserId', ownerUserId))
		.collect()
}
export function coresByGroup(ctx: QueryCtx, friendGroupId: string) {
	return ctx.db
		.query('cores')
		.withIndex('by_friend_group', (q) => q.eq('friendGroupId', friendGroupId))
		.collect()
}
export function coreListById(ctx: QueryCtx | MutationCtx, coreId: string) {
	return ctx.db
		.query('coreList')
		.withIndex('by_core_id', (q) => q.eq('coreId', coreId))
		.unique()
}

export function acceptedFriendship(ctx: QueryCtx | MutationCtx, a: string, b: string) {
	const [userAId, userBId] = canonicalPair(a, b)
	return ctx.db
		.query('friendships')
		.withIndex('by_pair', (q) => q.eq('userAId', userAId).eq('userBId', userBId))
		.unique()
}

export function friendRequestByPair(
	ctx: QueryCtx | MutationCtx,
	fromUserId: string,
	toUserId: string,
) {
	return ctx.db
		.query('friendRequests')
		.withIndex('by_from_to', (q) => q.eq('fromUserId', fromUserId).eq('toUserId', toUserId))
		.order('desc')
		.first()
}

export function blockByPair(
	ctx: QueryCtx | MutationCtx,
	blockerUserId: string,
	blockedUserId: string,
) {
	return ctx.db
		.query('blockedUsers')
		.withIndex('by_blocker_blocked', (q) =>
			q.eq('blockerUserId', blockerUserId).eq('blockedUserId', blockedUserId),
		)
		.unique()
}

export function canonicalPair(a: string, b: string): [string, string] {
	return a < b ? [a, b] : [b, a]
}
