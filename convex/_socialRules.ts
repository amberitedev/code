import { getAuthUserId } from '@convex-dev/auth/server'
import type { Id } from './_generated/dataModel'
import type { MutationCtx, QueryCtx } from './_generated/server'
export type FriendGroupRole = 'owner' | 'admin' | 'member'
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
	const group = await ctx.db.get(friendGroupId as any)
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
export function publicUser(user: any, includeFriendCode = false) {
	const avatarUrl = user.avatarUrl ?? user.image
	return {
		id: user._id,
		userId: user._id,
		username: user.username,
		displayName: user.displayName,
		name: user.displayName ?? user.username,
		image: user.image,
		avatar_url: avatarUrl ?? null,
		bio: user.bio ?? null,
		created: new Date(user._creationTime ?? Date.now()).toISOString(),
		...(includeFriendCode && user.friendCode ? { friendCode: user.friendCode } : {}),
	}
}
export function publicProfile(user: any) {
	const avatarUrl = user.avatarUrl ?? user.image
	return {
		id: user._id,
		userId: user._id,
		username: user.username,
		name: user.displayName ?? user.username,
		displayName: user.displayName,
		avatar_url: avatarUrl ?? null,
		image: user.image,
		bio: user.bio ?? null,
		created: new Date(user._creationTime ?? Date.now()).toISOString(),
		createdAt: user._creationTime ?? null,
		profileUpdatedAt: user.profileUpdatedAt ?? null,
	}
}
export function publicCurrentProfile(user: any) {
	return {
		...publicProfile(user),
		friendCode: user.friendCode,
		avatarStorageId: user.avatarStorageId,
		avatarMimeType: user.avatarMimeType,
		avatarSizeBytes: user.avatarSizeBytes,
		deletedAt: user.deletedAt,
		deletedReason: user.deletedReason,
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
