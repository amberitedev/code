import { v } from 'convex/values'
import type { Doc } from './_generated/dataModel'
import { query } from './_generated/server'
import type { QueryCtx } from './_generated/server'
import {
	currentAccountFields,
	minecraftUsername,
	minecraftUuid,
	requireUserId,
} from './_socialRules'

const campaignsValidator = v.object({ pride_26: v.null() })
const userValidator = v.object({
	id: v.string(),
	username: v.string(),
	display_name: v.string(),
	name: v.string(),
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
	display_name: v.string(),
	avatar_url: v.union(v.string(), v.null()),
})

export const currentUser = query({
	args: {},
	returns: userValidator,
	handler: async (ctx) => {
		const userId = await requireUserId(ctx)
		const user = await ctx.db.get(userId)
		if (!user || user.deletedAt) throw new Error('user not found')
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
		const user = await resolveUser(ctx, args.idOrUsername)
		return user && !user.deletedAt ? compatUser(user) : null
	},
})

export const getUsers = query({
	args: { ids: v.array(v.string()) },
	returns: v.array(userValidator),
	handler: async (ctx, args) => {
		const users = await Promise.all(args.ids.slice(0, 100).map((id) => resolveUser(ctx, id)))
		return users
			.filter((user): user is Doc<'users'> => Boolean(user && !user.deletedAt))
			.map((user) => compatUser(user))
	},
})

export const searchUsers = query({
	args: { query: v.string() },
	returns: v.array(searchUserValidator),
	handler: async (ctx, args) => {
		const normalized = args.query.trim().replace(/^@/, '').toLowerCase()
		if (normalized.length < 2) return []
		const users = await ctx.db
			.query('users')
			.withIndex('by_normalized_username', (index) =>
				index.gte('normalizedUsername', normalized).lt('normalizedUsername', `${normalized}\uffff`),
			)
			.take(20)
		return users
			.filter((user) => !user.deletedAt && Boolean(user.minecraftUuid && user.username))
			.map((user) => ({
				id: minecraftUuid(user),
				username: minecraftUsername(user),
				display_name: user.displayName ?? user.name ?? minecraftUsername(user),
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
	const username = minecraftUsername(user)
	return {
		id: minecraftUuid(user),
		username,
		display_name: user.displayName ?? user.name ?? username,
		name: user.displayName ?? user.name ?? username,
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
	const input = idOrUsername.trim().replace(/^@/, '')
	const uuid = input.replace(/-/g, '').toLowerCase()
	const userByUuid = await ctx.db
		.query('users')
		.withIndex('by_minecraft_uuid', (index) => index.eq('minecraftUuid', uuid))
		.unique()
	if (userByUuid) return userByUuid
	// TODO: When Minecraft rename history exists, optionally resolve previous names after re-verification.
	return await ctx.db
		.query('users')
		.withIndex('by_normalized_username', (index) =>
			index.eq('normalizedUsername', input.toLowerCase()),
		)
		.unique()
}
