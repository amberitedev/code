import { v } from 'convex/values'
import type { Doc } from './_generated/dataModel'
import { mutation, query } from './_generated/server'
import type { QueryCtx } from './_generated/server'
import {
	currentAccountFields,
	publicCurrentProfileValidator,
	publicCurrentProfile,
	publicProfile,
	publicProfileValidator,
	requireUserId,
} from './_socialRules'

const MAX_BIO_LENGTH = 160
const MAX_DISPLAY_NAME_LENGTH = 64
const MAX_SEARCH_LIMIT = 20

const avatarInput = v.union(
	v.null(),
	v.object({
		url: v.string(),
		storageId: v.optional(v.id('_storage')),
		mimeType: v.optional(v.string()),
		sizeBytes: v.optional(v.number()),
	}),
)

export const current = query({
	args: {},
	returns: publicCurrentProfileValidator,
	handler: async (ctx) => {
		const userId = await requireUserId(ctx)
		const user = await ctx.db.get(userId)
		if (!user || user.deletedAt) throw new Error('user not found')
		return publicCurrentProfile(user, await currentAccountFields(ctx, userId))
	},
})

export const get = query({
	args: { idOrUsername: v.string() },
	returns: v.union(publicProfileValidator, v.null()),
	handler: async (ctx, args) => {
		const user = await resolvePublicUser(ctx, args.idOrUsername)
		return user && !user.deletedAt ? publicProfile(user) : null
	},
})

export const view = query({
	args: { idOrUsername: v.string() },
	returns: v.union(v.object({ user: publicProfileValidator }), v.null()),
	handler: async (ctx, args) => {
		const user = await resolvePublicUser(ctx, args.idOrUsername)
		if (!user || user.deletedAt) return null
		return { user: publicProfile(user) }
	},
})

export const search = query({
	args: { query: v.string(), limit: v.optional(v.number()) },
	returns: v.array(publicProfileValidator),
	handler: async (ctx, args) => {
		const normalized = normalizeUsernameQuery(args.query)
		if (normalized.length < 2) return []
		const limit = normalizeLimit(args.limit)
		const users = await ctx.db
			.query('users')
			.withIndex('by_normalized_username', (index) =>
				index.gte('normalizedUsername', normalized).lt('normalizedUsername', `${normalized}\uffff`),
			)
			.take(limit)
		return users
			.filter((user) => !user.deletedAt && user.minecraftUuid && user.username)
			.map(publicProfile)
	},
})

export const updateCurrent = mutation({
	args: {
		displayName: v.optional(v.string()),
		bio: v.optional(v.union(v.string(), v.null())),
		avatar: v.optional(avatarInput),
		allowFriendRequests: v.optional(v.boolean()),
	},
	returns: publicCurrentProfileValidator,
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)
		const user = await ctx.db.get(userId)
		if (!user || user.deletedAt) throw new Error('user not found')

		const patch: Partial<Doc<'users'>> = { profileUpdatedAt: Date.now() }
		if (args.displayName !== undefined) {
			const displayName = normalizeDisplayName(args.displayName)
			patch.displayName = displayName
			patch.name = displayName
		}
		if (args.bio !== undefined) patch.bio = args.bio === null ? undefined : normalizeBio(args.bio)
		if (args.allowFriendRequests !== undefined) patch.allowFriendRequests = args.allowFriendRequests
		if (args.avatar !== undefined) {
			if (args.avatar === null) {
				patch.avatarUrl = undefined
				patch.avatarStorageId = undefined
				patch.avatarMimeType = undefined
				patch.avatarSizeBytes = undefined
			} else {
				patch.avatarUrl = normalizeAvatarUrl(args.avatar.url)
				patch.avatarStorageId = args.avatar.storageId
				patch.avatarMimeType = args.avatar.mimeType
				patch.avatarSizeBytes = normalizeAvatarSize(args.avatar.sizeBytes)
			}
		}

		await ctx.db.patch(userId, patch)
		const updated = await ctx.db.get(userId)
		if (!updated) throw new Error('user not found')
		return publicCurrentProfile(updated, await currentAccountFields(ctx, userId))
	},
})

async function resolvePublicUser(ctx: QueryCtx, value: string): Promise<Doc<'users'> | null> {
	const input = value.trim().replace(/^@/, '')
	const byUuid = await ctx.db
		.query('users')
		.withIndex('by_minecraft_uuid', (index) => index.eq('minecraftUuid', normalizeUuid(input)))
		.unique()
	if (byUuid) return byUuid
	// Minecraft usernames can change. Until rename history and re-verification are implemented,
	// only the currently verified username resolves; the stable public ID remains the UUID.
	return await ctx.db
		.query('users')
		.withIndex('by_normalized_username', (index) =>
			index.eq('normalizedUsername', input.toLowerCase()),
		)
		.unique()
}

function normalizeUuid(value: string): string {
	return value.replace(/-/g, '').toLowerCase()
}

function normalizeUsernameQuery(value: string): string {
	return value.trim().replace(/^@/, '').toLowerCase()
}

function normalizeLimit(value: number | undefined): number {
	if (value === undefined) return 8
	if (!Number.isInteger(value) || value < 1 || value > MAX_SEARCH_LIMIT)
		throw new Error(`limit must be between 1 and ${MAX_SEARCH_LIMIT}`)
	return value
}

function normalizeDisplayName(value: string): string {
	const displayName = value.trim()
	if (!displayName || displayName.length > MAX_DISPLAY_NAME_LENGTH)
		throw new Error(`display name must be between 1 and ${MAX_DISPLAY_NAME_LENGTH} characters`)
	return displayName
}

function normalizeBio(value: string): string {
	const bio = value.trim()
	if (bio.length > MAX_BIO_LENGTH)
		throw new Error(`bio must be ${MAX_BIO_LENGTH} characters or fewer`)
	return bio
}

function normalizeAvatarUrl(value: string): string {
	const url = value.trim()
	if (/^data:image\/(png|jpeg|gif|webp);base64,[a-zA-Z0-9+/=]+$/.test(url)) {
		if (url.length > 400_000) throw new Error('avatar image is too large')
		return url
	}
	if (url.length > 2_048) throw new Error('avatar URL is too long')
	const parsed = new URL(url)
	if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:')
		throw new Error('avatar URL must use HTTP or HTTPS')
	return parsed.toString()
}

function normalizeAvatarSize(value: number | undefined): number | undefined {
	if (value === undefined) return undefined
	if (!Number.isInteger(value) || value < 0 || value > 262_144)
		throw new Error('avatar must be 256 KiB or smaller')
	return value
}
