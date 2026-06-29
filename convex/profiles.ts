import { v } from 'convex/values'
import { mutation, query } from './_generated/server'
import type { Id } from './_generated/dataModel'
import type { MutationCtx, QueryCtx } from './_generated/server'
import {
	currentAccountFields,
	publicCurrentProfile,
	publicProfile,
	requireUserId,
} from './_socialRules'

const MAX_BIO_LENGTH = 1_024
const MAX_SEARCH_LIMIT = 20
const DEFAULT_SEARCH_LIMIT = 8
const USERNAME_PATTERN = /^[a-zA-Z0-9_]{3,24}$/

const avatarInput = v.union(
	v.null(),
	v.object({
		url: v.string(),
		storageId: v.optional(v.string()),
		mimeType: v.optional(v.string()),
		sizeBytes: v.optional(v.number()),
	}),
)

export const current = query({
	args: {},
	handler: async (ctx) => {
		const userId = await requireUserId(ctx)
		const user = await ctx.db.get(userId)
		if (!user) throw new Error('user not found')
		return publicCurrentProfile(user, await currentAccountFields(ctx, userId))
	},
})

export const get = query({
	args: { idOrUsername: v.string() },
	handler: async (ctx, args) => {
		await requireUserId(ctx)
		const user = await findUser(ctx, args.idOrUsername)
		return user && !user.deletedAt ? publicProfile(user) : null
	},
})

export const search = query({
	args: { query: v.string(), limit: v.optional(v.number()) },
	handler: async (ctx, args) => {
		const viewerId = await requireUserId(ctx)
		const term = args.query.trim()
		if (term.length < 2 || term.length > 32) return []
		const limit = strictLimit(args.limit)
		const normalizedTerm = term.toLowerCase()
		const upperBound = `${normalizedTerm}\uffff`
		const users = await ctx.db
			.query('users')
			.withIndex('by_normalized_username', (q) =>
				q.gte('normalizedUsername', normalizedTerm).lt('normalizedUsername', upperBound),
			)
			.take(limit)
		return users
			.filter((user) => {
				if (user._id === viewerId || user.deletedAt) return false
				return user.normalizedUsername?.startsWith(normalizedTerm)
			})
			.map((user) => publicProfile(user))
	},
})

export const updateCurrent = mutation({
	args: {
		displayName: v.optional(v.string()),
		username: v.optional(v.string()),
		bio: v.optional(v.string()),
		avatar: v.optional(avatarInput),
	},
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)
		const user = await ctx.db.get(userId)
		if (!user) throw new Error('user not found')
		const patch: any = { profileUpdatedAt: Date.now() }

		if (args.username !== undefined) {
			const username = normalizeUsername(args.username)
			const existing = await userByUsername(ctx, username)
			if (existing && existing._id !== userId) throw new Error('username is already taken')
			patch.username = username
			patch.normalizedUsername = username.toLowerCase()
			patch.onboardedAt = user.onboardedAt ?? Date.now()
			patch.amberiteUserId = user.amberiteUserId ?? crypto.randomUUID()
		}

		if (args.displayName !== undefined) {
			const displayName = normalizeDisplayName(args.displayName)
			patch.displayName = displayName
			patch.name = displayName
			patch.onboardedAt = user.onboardedAt ?? Date.now()
			patch.amberiteUserId = user.amberiteUserId ?? crypto.randomUUID()
		} else if (args.username !== undefined && !user.displayName) {
			patch.displayName = patch.username
			patch.name = patch.username
		}

		if (args.bio !== undefined) patch.bio = normalizeBio(args.bio)

		if (args.avatar !== undefined) {
			if (args.avatar === null) {
				patch.avatarUrl = undefined
				patch.avatarStorageId = undefined
				patch.avatarMimeType = undefined
				patch.avatarSizeBytes = undefined
				patch.image = undefined
			} else {
				patch.avatarUrl = normalizeAvatarUrl(args.avatar.url)
				patch.avatarStorageId = args.avatar.storageId
				patch.avatarMimeType = args.avatar.mimeType
				patch.avatarSizeBytes = args.avatar.sizeBytes
				patch.image = patch.avatarUrl
			}
		}

		await ctx.db.patch(userId, patch)
		const updated = await ctx.db.get(userId)
		if (!updated) throw new Error('user not found')
		return publicCurrentProfile(updated, await currentAccountFields(ctx, userId))
	},
})

async function findUser(ctx: QueryCtx, idOrUsername: string) {
	const normalized = idOrUsername.trim().toLowerCase()
	const byUsername = await ctx.db
		.query('users')
		.withIndex('by_normalized_username', (q) => q.eq('normalizedUsername', normalized))
		.unique()
	if (byUsername) return byUsername
	try {
		return await ctx.db.get(idOrUsername as Id<'users'>)
	} catch {
		return null
	}
}

function userByUsername(ctx: QueryCtx | MutationCtx, username: string) {
	return ctx.db
		.query('users')
		.withIndex('by_normalized_username', (q) => q.eq('normalizedUsername', username.toLowerCase()))
		.unique()
}

function normalizeUsername(value: string): string {
	const username = value.trim()
	if (!USERNAME_PATTERN.test(username))
		throw new Error('username must be 3-24 letters, numbers, or underscores')
	return username
}

function normalizeDisplayName(value: string): string {
	const displayName = value.trim()
	if (displayName.length < 1 || displayName.length > 64)
		throw new Error('display name must be between 1 and 64 characters')
	return displayName
}

function normalizeBio(value: string): string {
	const bio = value.trim()
	if (bio.length > MAX_BIO_LENGTH) throw new Error(`bio must be ${MAX_BIO_LENGTH} characters or fewer`)
	return bio
}

function normalizeAvatarUrl(value: string): string {
	const url = value.trim()
	if (/^data:image\/(png|jpeg|gif|webp);base64,[a-zA-Z0-9+/=]+$/.test(url)) {
		if (url.length > 400_000) throw new Error('avatar image is too large')
		return url
	}
	if (url.length > 2_048) throw new Error('avatar URL is too long')
	try {
		const parsed = new URL(url)
		if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:') throw new Error()
	} catch {
		throw new Error('avatar URL must be a valid HTTP or HTTPS URL')
	}
	return url
}

function strictLimit(value: number | undefined): number {
	if (value === undefined) return DEFAULT_SEARCH_LIMIT
	if (!Number.isInteger(value) || value < 1 || value > MAX_SEARCH_LIMIT)
		throw new Error(`limit must be between 1 and ${MAX_SEARCH_LIMIT}`)
	return value
}
