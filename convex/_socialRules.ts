import { getAuthUserId } from '@convex-dev/auth/server'
import { v } from 'convex/values'
import type { Doc, Id } from './_generated/dataModel'
import type { MutationCtx, QueryCtx } from './_generated/server'

const publicUserFields = {
	id: v.string(),
	userId: v.string(),
	username: v.string(),
	display_name: v.string(),
	displayName: v.string(),
	name: v.string(),
	minecraftUuid: v.string(),
	verifiedMinecraftHandle: v.string(),
	avatar_url: v.union(v.string(), v.null()),
	image: v.optional(v.string()),
	bio: v.union(v.string(), v.null()),
	created: v.string(),
	friendCode: v.optional(v.string()),
	allow_friend_requests: v.boolean(),
	email: v.optional(v.string()),
	email_verified: v.optional(v.boolean()),
	auth_providers: v.optional(v.array(v.string())),
	has_password: v.optional(v.boolean()),
	has_totp: v.optional(v.boolean()),
}

export const publicUserValidator = v.object(publicUserFields)
export const publicProfileValidator = v.object({
	...publicUserFields,
	createdAt: v.number(),
	profileUpdatedAt: v.union(v.number(), v.null()),
})
export const publicCurrentProfileValidator = v.object({
	...publicUserFields,
	createdAt: v.number(),
	profileUpdatedAt: v.union(v.number(), v.null()),
	avatarStorageId: v.optional(v.string()),
	avatarMimeType: v.optional(v.string()),
	avatarSizeBytes: v.optional(v.number()),
	deletedAt: v.optional(v.number()),
	deletedReason: v.optional(v.string()),
})

export async function requireUserId(ctx: QueryCtx | MutationCtx): Promise<Id<'users'>> {
	const userId = await getAuthUserId(ctx)
	if (userId === null) throw new Error('not authenticated')
	return userId
}

export async function resolveActor(
	ctx: QueryCtx | MutationCtx,
	devUserId?: string,
): Promise<Id<'users'>> {
	if (devUserId !== undefined) {
		if (!isDevMode()) throw new Error('development acting-user overrides are disabled')
		const normalized = ctx.db.normalizeId('users', devUserId)
		if (!normalized || !(await ctx.db.get(normalized)))
			throw new Error('development user not found')
		return normalized
	}
	return await requireUserId(ctx)
}

export function isDevMode(): boolean {
	return process.env.AMBERITE_DEV_MODE === 'true'
}

export function canonicalPair(first: Id<'users'>, second: Id<'users'>): [Id<'users'>, Id<'users'>] {
	return first < second ? [first, second] : [second, first]
}

export function acceptedFriendship(
	ctx: QueryCtx | MutationCtx,
	first: Id<'users'>,
	second: Id<'users'>,
) {
	const [userAId, userBId] = canonicalPair(first, second)
	return ctx.db
		.query('friendships')
		.withIndex('by_pair', (query) => query.eq('userAId', userAId).eq('userBId', userBId))
		.unique()
}

export function friendRequestByPair(
	ctx: QueryCtx | MutationCtx,
	fromUserId: Id<'users'>,
	toUserId: Id<'users'>,
) {
	return ctx.db
		.query('friendRequests')
		.withIndex('by_from_to', (query) => query.eq('fromUserId', fromUserId).eq('toUserId', toUserId))
		.order('desc')
		.first()
}

export function blockByPair(
	ctx: QueryCtx | MutationCtx,
	blockerUserId: Id<'users'>,
	blockedUserId: Id<'users'>,
) {
	return ctx.db
		.query('blockedUsers')
		.withIndex('by_blocker_blocked', (query) =>
			query.eq('blockerUserId', blockerUserId).eq('blockedUserId', blockedUserId),
		)
		.unique()
}

export function minecraftUuid(user: Doc<'users'>): string {
	if (!user.minecraftUuid) throw new Error('user has no verified Minecraft identity')
	return user.minecraftUuid
}

export function minecraftUsername(user: Doc<'users'>): string {
	const username = user.verifiedMinecraftHandle ?? user.username
	if (!username) throw new Error('user has no verified Minecraft identity')
	return username
}

export function publicUser(
	user: Doc<'users'>,
	includePrivate = false,
	privateFields?: Awaited<ReturnType<typeof currentAccountFields>>,
) {
	const id = minecraftUuid(user)
	const username = minecraftUsername(user)
	const displayName = user.displayName ?? user.name ?? username
	const avatarUrl = user.avatarUrl ?? user.image ?? null
	return {
		id,
		userId: user._id.toString(),
		username,
		display_name: displayName,
		displayName,
		name: displayName,
		minecraftUuid: id,
		verifiedMinecraftHandle: username,
		avatar_url: avatarUrl,
		image: avatarUrl ?? undefined,
		bio: user.bio ?? null,
		created: new Date(user._creationTime).toISOString(),
		friendCode: includePrivate ? user.friendCode : undefined,
		allow_friend_requests: user.allowFriendRequests ?? true,
		...(includePrivate && privateFields
			? {
					email: privateFields.email ?? undefined,
					email_verified: privateFields.email_verified,
					auth_providers: privateFields.auth_providers,
					has_password: privateFields.has_password,
					has_totp: privateFields.has_totp,
				}
			: {}),
	}
}

export function publicProfile(user: Doc<'users'>) {
	return {
		...publicUser(user),
		createdAt: user._creationTime,
		profileUpdatedAt: user.profileUpdatedAt ?? null,
	}
}

export function publicCurrentProfile(
	user: Doc<'users'>,
	accountFields: Awaited<ReturnType<typeof currentAccountFields>>,
) {
	return {
		...publicProfile(user),
		...publicUser(user, true, accountFields),
		avatarStorageId: user.avatarStorageId?.toString(),
		avatarMimeType: user.avatarMimeType,
		avatarSizeBytes: user.avatarSizeBytes,
		deletedAt: user.deletedAt,
		deletedReason: user.deletedReason,
	}
}

export async function currentAccountFields(ctx: QueryCtx | MutationCtx, userId: Id<'users'>) {
	const [user, accounts] = await Promise.all([
		ctx.db.get(userId),
		ctx.db
			.query('authAccounts')
			.withIndex('userIdAndProvider', (query) => query.eq('userId', userId))
			.collect(),
	])
	return {
		email: user?.email ?? null,
		email_verified: Boolean(user?.emailVerificationTime),
		auth_providers: [...new Set(accounts.map((account) => account.provider))],
		has_password: accounts.some((account) => account.secret !== undefined),
		has_totp: false,
	}
}
