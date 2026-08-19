import { getAuthSessionId } from '@convex-dev/auth/server'
import { v } from 'convex/values'
import { mutation, query } from './_generated/server'
import { minecraftUuid, requireUserId } from './_socialRules'

const sessionValidator = v.object({
	id: v.string(),
	session: v.null(),
	user_id: v.string(),
	created: v.string(),
	last_login: v.string(),
	expires: v.string(),
	refresh_expires: v.string(),
	os: v.union(v.string(), v.null()),
	platform: v.union(v.string(), v.null()),
	user_agent: v.string(),
	city: v.union(v.string(), v.null()),
	country: v.union(v.string(), v.null()),
	ip: v.string(),
	current: v.boolean(),
})

export const registerCurrent = mutation({
	args: {
		os: v.optional(v.string()),
		platform: v.optional(v.string()),
		userAgent: v.string(),
		city: v.optional(v.string()),
		country: v.optional(v.string()),
		ip: v.optional(v.string()),
	},
	returns: v.null(),
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)
		const authSessionId = await getAuthSessionId(ctx)
		if (!authSessionId) throw new Error('not authenticated')
		const existing = await ctx.db
			.query('deviceSessions')
			.withIndex('by_auth_session', (index) => index.eq('authSessionId', authSessionId))
			.unique()
		const value = {
			userId,
			authSessionId,
			lastSeenAt: Date.now(),
			os: normalizeOptional(args.os, 128),
			platform: normalizeOptional(args.platform, 128),
			userAgent: normalizeRequired(args.userAgent, 512, 'user agent'),
			city: normalizeOptional(args.city, 128),
			country: normalizeOptional(args.country, 64),
			ip: normalizeOptional(args.ip, 64),
		}
		if (existing) await ctx.db.patch(existing._id, value)
		else await ctx.db.insert('deviceSessions', value)
		return null
	},
})

export const list = query({
	args: { now: v.number() },
	returns: v.array(sessionValidator),
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)
		const currentSessionId = await getAuthSessionId(ctx)
		const user = await ctx.db.get(userId)
		if (!user) throw new Error('user not found')
		const [authSessions, metadata] = await Promise.all([
			ctx.db
				.query('authSessions')
				.withIndex('userId', (index) => index.eq('userId', userId))
				.take(100),
			ctx.db
				.query('deviceSessions')
				.withIndex('by_user', (index) => index.eq('userId', userId))
				.take(100),
		])
		const metadataBySession = new Map(metadata.map((entry) => [entry.authSessionId, entry]))
		return authSessions
			.filter((session) => session.expirationTime > args.now)
			.map((session) => {
				const details = metadataBySession.get(session._id)
				const created = new Date(session._creationTime).toISOString()
				const expires = new Date(session.expirationTime).toISOString()
				return {
					id: session._id.toString(),
					session: null,
					user_id: minecraftUuid(user),
					created,
					last_login: new Date(details?.lastSeenAt ?? session._creationTime).toISOString(),
					expires,
					refresh_expires: expires,
					os: details?.os ?? null,
					platform: details?.platform ?? null,
					user_agent: details?.userAgent ?? 'Amberite (unregistered device)',
					city: details?.city ?? null,
					country: details?.country ?? null,
					ip: details?.ip ?? '',
					current: session._id === currentSessionId,
				}
			})
	},
})

export const revoke = mutation({
	args: { id: v.id('authSessions') },
	returns: v.null(),
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)
		const session = await ctx.db.get(args.id)
		if (!session || session.userId !== userId) return null
		const refreshTokens = await ctx.db
			.query('authRefreshTokens')
			.withIndex('sessionId', (index) => index.eq('sessionId', args.id))
			.collect()
		for (const refreshToken of refreshTokens) await ctx.db.delete(refreshToken._id)
		const metadata = await ctx.db
			.query('deviceSessions')
			.withIndex('by_auth_session', (index) => index.eq('authSessionId', args.id))
			.unique()
		if (metadata) await ctx.db.delete(metadata._id)
		await ctx.db.delete(args.id)
		return null
	},
})

function normalizeOptional(value: string | undefined, max: number): string | undefined {
	if (value === undefined) return undefined
	const normalized = value.trim()
	if (!normalized) return undefined
	if (normalized.length > max) throw new Error('session metadata is too long')
	return normalized
}

function normalizeRequired(value: string, max: number, field: string): string {
	const normalized = value.trim()
	if (!normalized || normalized.length > max) throw new Error(`invalid ${field}`)
	return normalized
}
