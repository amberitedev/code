import { v } from 'convex/values'
import type { Doc, Id } from './_generated/dataModel'
import { query } from './_generated/server'
import type { QueryCtx } from './_generated/server'
import { currentAccountFields, requireUserId } from './_socialRules'

const campaignsValidator = v.object({ pride_26: v.null() })
const userValidator = v.object({
	id: v.string(),
	username: v.string(),
	avatar_url: v.optional(v.string()),
	bio: v.optional(v.string()),
	created: v.string(),
	role: v.literal('developer'),
	badges: v.number(),
	campaigns: campaignsValidator,
	auth_providers: v.optional(v.array(v.literal('microsoft'))),
	email: v.optional(v.string()),
	email_verified: v.optional(v.boolean()),
	has_password: v.optional(v.boolean()),
	has_totp: v.optional(v.boolean()),
	allow_friend_requests: v.optional(v.boolean()),
})
const searchUserValidator = v.object({
	id: v.string(),
	username: v.string(),
	avatar_url: v.union(v.string(), v.null()),
})

export const currentUser = query({
	args: {},
	returns: userValidator,
	handler: async (ctx) => {
		const userId = await requireUserId(ctx)
		const user = await ctx.db.get(userId)
		if (!user) throw new Error('user not found')
		const fields = await currentAccountFields(ctx, userId)
		return compatUser(user, {
			authProviders: fields.auth_providers.includes('minecraft') ? ['microsoft'] : [],
			email: fields.email,
			emailVerified: fields.email_verified,
		})
	},
})

export const getUser = query({
	args: { idOrUsername: v.string() },
	returns: v.union(userValidator, v.null()),
	handler: async (ctx, args) => {
		await requireUserId(ctx)
		const user = await resolveUser(ctx, args.idOrUsername)
		return user && !user.deletedAt ? compatUser(user) : null
	},
})

export const getUsers = query({
	args: { ids: v.array(v.id('users')) },
	returns: v.array(userValidator),
	handler: async (ctx, args) => {
		await requireUserId(ctx)
		const users = await Promise.all(args.ids.slice(0, 100).map((id) => ctx.db.get(id)))
		return users
			.filter((user): user is Doc<'users'> => Boolean(user && !user.deletedAt))
			.map((user) => compatUser(user))
	},
})

export const searchUsers = query({
	args: { query: v.string() },
	returns: v.array(searchUserValidator),
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		const normalized = args.query.trim().toLowerCase()
		if (normalized.length < 2) return []
		const users = await ctx.db
			.query('users')
			.withIndex('by_verified_minecraft_handle', (q) =>
				q
					.gte('normalizedVerifiedMinecraftHandle', normalized)
					.lt('normalizedVerifiedMinecraftHandle', `${normalized}\uffff`),
			)
			.take(20)
		return users
			.filter(
				(user) => user._id !== actorId && !user.deletedAt && Boolean(user.verifiedMinecraftHandle),
			)
			.map((user) => ({
				id: user._id.toString(),
				username: user.verifiedMinecraftHandle!,
				avatar_url: user.avatarUrl ?? user.image ?? null,
			}))
	},
})

function compatUser(
	user: Doc<'users'>,
	privateFields?: {
		authProviders: 'microsoft'[]
		email: string | null
		emailVerified: boolean
	},
) {
	const username = user.verifiedMinecraftHandle ?? user.username
	if (!username) throw new Error('user has no verified Minecraft identity')
	return {
		id: user._id.toString(),
		username,
		...((user.avatarUrl ?? user.image) ? { avatar_url: user.avatarUrl ?? user.image } : {}),
		...(user.bio ? { bio: user.bio } : {}),
		created: new Date(user._creationTime).toISOString(),
		role: 'developer' as const,
		badges: 0,
		campaigns: { pride_26: null },
		allow_friend_requests: user.allowFriendRequests ?? true,
		...(privateFields
			? {
					auth_providers: privateFields.authProviders,
					...(privateFields.email ? { email: privateFields.email } : {}),
					email_verified: privateFields.emailVerified,
					has_password: false,
					has_totp: false,
				}
			: {}),
	}
}

async function resolveUser(ctx: QueryCtx, idOrUsername: string): Promise<Doc<'users'> | null> {
	const userId = ctx.db.normalizeId('users', idOrUsername)
	if (userId) return await ctx.db.get(userId)
	return await ctx.db
		.query('users')
		.withIndex('by_verified_minecraft_handle', (q) =>
			q.eq('normalizedVerifiedMinecraftHandle', idOrUsername.trim().toLowerCase()),
		)
		.unique()
}
