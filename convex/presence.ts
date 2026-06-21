import { v } from 'convex/values'
import { mutation, query } from './_generated/server'
import type { MutationCtx, QueryCtx } from './_generated/server'
import {
	coreById,
	ensureFriendGroupCore,
	getOrCreateDefaultFriendGroup,
	requireFriendGroupRole,
	requireSingleGroupMembership,
	requireSingleOwnedCore,
	resolveActor,
} from './_socialRules'

/** Optional dev-only acting-user override, honoured only when AMBERITE_DEV_MODE is set. */
const devActAs = { __actAs: v.optional(v.string()) }

export const registerCore = mutation({
	args: {
		coreId: v.string(),
		ownerUserId: v.string(),
		friendGroupId: v.optional(v.string()),
		connectionUrl: v.optional(v.string()),
		status: v.optional(v.string()),
		metadata: v.optional(v.any()),
		...devActAs,
	},
	returns: v.object({ coreId: v.string() }),
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
		if (args.ownerUserId !== userId) throw new Error('cannot register a Core for another user')
		const now = Date.now()
		const existing = await coreById(ctx, args.coreId)
		let friendGroupId = args.friendGroupId
		if (friendGroupId) {
			await requireFriendGroupRole(ctx, userId, friendGroupId, ['owner'])
			await requireSingleGroupMembership(ctx, userId, friendGroupId)
			await requireSingleOwnedCore(ctx, userId, args.coreId)
			await ensureFriendGroupCore(ctx, friendGroupId, args.coreId, now)
		} else {
			friendGroupId = await getOrCreateDefaultFriendGroup(ctx, userId, args.coreId, now)
		}

		if (existing) {
			await requireCoreRole(ctx, userId, args.coreId, ['owner', 'admin'])
			if (existing.ownerUserId !== userId) throw new Error('Core already belongs to another user')
			if (existing.friendGroupId && existing.friendGroupId !== friendGroupId)
				throw new Error('Core already belongs to another friend group')
			await ctx.db.patch(existing._id, {
				ownerUserId: userId,
				friendGroupId,
				connectionUrl: args.connectionUrl,
				status: args.status,
				metadata: args.metadata,
				lastSeenAt: now,
			})
		} else {
			await ctx.db.insert('cores', {
				coreId: args.coreId,
				ownerUserId: userId,
				friendGroupId,
				connectionUrl: args.connectionUrl,
				status: args.status,
				metadata: args.metadata,
				lastSeenAt: now,
			})
		}

		return { coreId: args.coreId }
	},
})

export const corePresence = query({
	args: { coreId: v.string(), ...devActAs },
	returns: v.union(
		v.null(),
		v.object({
			coreId: v.string(),
			ownerUserId: v.string(),
			friendGroupId: v.optional(v.string()),
			connectionUrl: v.optional(v.string()),
			lastSeenAt: v.number(),
			status: v.optional(v.string()),
			metadata: v.optional(v.any()),
		}),
	),
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
		const core = await ctx.db
			.query('cores')
			.withIndex('by_core_id', (q) => q.eq('coreId', args.coreId))
			.unique()
		if (!core) return null
		await requireCoreAccess(ctx, userId, core)
		return {
			coreId: core.coreId,
			ownerUserId: core.ownerUserId,
			friendGroupId: core.friendGroupId,
			connectionUrl: core.connectionUrl,
			lastSeenAt: core.lastSeenAt,
			status: core.status,
			metadata: core.metadata,
		}
	},
})

export const registerPairingCore = mutation({
	args: {
		code: v.string(),
		coreId: v.string(),
		connectionUrl: v.optional(v.string()),
		metadata: v.optional(v.any()),
		ttlMs: v.optional(v.number()),
	},
	returns: v.object({ coreId: v.string(), code: v.string() }),
	handler: async (ctx, args) => {
		const code = normalizePairingCode(args.code)
		if (!code) throw new Error('invalid pairing code format')
		const now = Date.now()
		await removeExpiredPairingCores(ctx, now)
		const existing = await ctx.db
			.query('pairingCores')
			.withIndex('by_core_id', (q) => q.eq('coreId', args.coreId))
			.unique()
		const value = {
			code,
			coreId: args.coreId,
			connectionUrl: args.connectionUrl,
			status: 'waiting' as const,
			metadata: args.metadata,
			createdAt: now,
			expiresAt: now + (args.ttlMs ?? 15 * 60 * 1000),
		}

		if (existing) await ctx.db.patch(existing._id, value)
		else await ctx.db.insert('pairingCores', value)
		return { coreId: args.coreId, code }
	},
})

export const claimPairingCore = mutation({
	args: { code: v.string(), ...devActAs },
	returns: v.union(
		v.null(),
		v.object({
			coreId: v.string(),
			connectionUrl: v.optional(v.string()),
			metadata: v.optional(v.any()),
		}),
	),
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
		const code = normalizePairingCode(args.code)
		if (!code) return null
		const now = Date.now()
		await removeExpiredPairingCores(ctx, now)
		const pairing = await ctx.db
			.query('pairingCores')
			.withIndex('by_code', (q) => q.eq('code', code))
			.unique()
		if (!pairing || pairing.status !== 'waiting' || pairing.expiresAt <= now) return null

		const existingCore = await coreById(ctx, pairing.coreId)
		if (existingCore && existingCore.ownerUserId !== userId)
			throw new Error('Core already belongs to another user')

		await ctx.db.patch(pairing._id, {
			status: 'claimed',
			ownerUserId: userId,
			claimedAt: now,
		})
		return {
			coreId: pairing.coreId,
			connectionUrl: pairing.connectionUrl,
			metadata: pairing.metadata,
		}
	},
})

export const finalizePairingCore = mutation({
	args: { code: v.string(), coreId: v.string(), connectionUrl: v.optional(v.string()), ...devActAs },
	returns: v.object({ coreId: v.string(), friendGroupId: v.string() }),
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
		const code = normalizePairingCode(args.code)
		if (!code) throw new Error('invalid pairing code format')
		const now = Date.now()
		await removeExpiredPairingCores(ctx, now)
		const pairing = await ctx.db
			.query('pairingCores')
			.withIndex('by_code', (q) => q.eq('code', code))
			.unique()
		if (
			!pairing ||
			pairing.status !== 'claimed' ||
			pairing.coreId !== args.coreId ||
			pairing.ownerUserId !== userId ||
			pairing.expiresAt <= now
		) {
			throw new Error('pairing claim not found')
		}

		const existingCore = await coreById(ctx, pairing.coreId)
		if (existingCore && existingCore.ownerUserId !== userId)
			throw new Error('Core already belongs to another user')
		const friendGroup = await getOrCreateDefaultFriendGroup(ctx, userId, pairing.coreId, now)
		if (existingCore?.friendGroupId && existingCore.friendGroupId !== friendGroup)
			throw new Error('Core already belongs to another friend group')

		const coreValue = {
			coreId: pairing.coreId,
			ownerUserId: userId,
			friendGroupId: friendGroup,
			connectionUrl: args.connectionUrl ?? pairing.connectionUrl,
			lastSeenAt: now,
			status: 'paired',
			metadata: pairing.metadata,
		}
		if (existingCore) await ctx.db.patch(existingCore._id, coreValue)
		else await ctx.db.insert('cores', coreValue)
		await ctx.db.delete(pairing._id)
		return { coreId: pairing.coreId, friendGroupId: friendGroup }
	},
})

export const releasePairingCore = mutation({
	args: { code: v.string(), coreId: v.string(), ...devActAs },
	returns: v.null(),
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
		const code = normalizePairingCode(args.code)
		if (!code) return null
		const pairing = await ctx.db
			.query('pairingCores')
			.withIndex('by_code', (q) => q.eq('code', code))
			.unique()
		if (pairing?.status === 'claimed' && pairing.coreId === args.coreId && pairing.ownerUserId === userId) {
			await ctx.db.patch(pairing._id, {
				status: 'waiting',
				ownerUserId: undefined,
				claimedAt: undefined,
			})
		}
		return null
	},
})

function normalizePairingCode(code: string): string | null {
	const normalized = code.trim().replace(/[^a-z0-9]/gi, '').toLowerCase()
	return /^[a-hj-np-z2-9]{8}$/.test(normalized) ? normalized : null
}

async function removeExpiredPairingCores(ctx: MutationCtx, now: number): Promise<void> {
	const expired = await ctx.db
		.query('pairingCores')
		.withIndex('by_expires_at', (q) => q.lte('expiresAt', now))
		.take(10)
	await Promise.all(expired.map((core) => ctx.db.delete(core._id)))
}

async function requireCoreRole(
	ctx: QueryCtx | MutationCtx,
	userId: string,
	coreId: string,
	allowedRoles: Array<'owner' | 'admin' | 'member'>,
): Promise<void> {
	const core = await coreById(ctx, coreId)
	if (!core) throw new Error('Core not found')
	await requireCoreAccess(ctx, userId, core, allowedRoles)
}

async function requireCoreAccess(
	ctx: QueryCtx | MutationCtx,
	userId: string,
	core: { ownerUserId: string; friendGroupId?: string },
	allowedRoles: Array<'owner' | 'admin' | 'member'> = ['owner', 'admin', 'member'],
): Promise<void> {
	if (core.ownerUserId === userId && allowedRoles.includes('owner')) return
	if (!core.friendGroupId) throw new Error('not authorized for Core')
	await requireFriendGroupRole(ctx, userId, core.friendGroupId, allowedRoles)
}
