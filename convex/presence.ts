import { v } from 'convex/values'
import { mutation } from './_generated/server'
import type { MutationCtx } from './_generated/server'
import { requireUserId } from './_socialRules'

const PAIRING_MIN_TTL_MS = 60_000
const PAIRING_MAX_TTL_MS = 15 * 60 * 1000
const PAIRING_REREGISTER_COOLDOWN_MS = 5_000
const MAX_CONNECTION_URL_LENGTH = 2_048
const MAX_BIND_HOST_LENGTH = 128
const pairingMetadataValidator = v.object({
	bindHost: v.optional(v.string()),
	port: v.optional(v.number()),
})

/** The unauthenticated Core half of explicit pairing. This is registration, not discovery. */
export const registerPairingCore = mutation({
	args: {
		code: v.string(),
		coreId: v.string(),
		connectionUrl: v.optional(v.string()),
		metadata: v.optional(pairingMetadataValidator),
		ttlMs: v.optional(v.number()),
	},
	returns: v.object({ coreId: v.string(), code: v.string() }),
	handler: async (ctx, args) => {
		const code = normalizePairingCode(args.code)
		if (!code) throw new Error('invalid pairing code format')
		if (!validId(args.coreId)) throw new Error('invalid Core id')
		if (
			args.connectionUrl !== undefined &&
			!validOptionalString(args.connectionUrl, MAX_CONNECTION_URL_LENGTH)
		)
			throw new Error('invalid connection URL')
		const metadata = normalizePairingMetadata(args.metadata)
		const now = Date.now()
		await removeExpiredPairingCores(ctx, now)
		const existing = await ctx.db
			.query('pairingCores')
			.withIndex('by_core_id', (index) => index.eq('coreId', args.coreId))
			.unique()
		if (existing && now - existing.createdAt < PAIRING_REREGISTER_COOLDOWN_MS)
			throw new Error('pairing registration is cooling down')
		const value = {
			code,
			coreId: args.coreId,
			connectionUrl: args.connectionUrl,
			status: 'waiting' as const,
			metadata,
			createdAt: now,
			expiresAt: now + clampTtl(args.ttlMs),
			ownerUserId: undefined,
			claimedAt: undefined,
			realtimeCredentialHash: undefined,
			realtimeCredentialIssuedAt: undefined,
		}
		if (existing) await ctx.db.patch(existing._id, value)
		else await ctx.db.insert('pairingCores', value)
		return { coreId: args.coreId, code }
	},
})

export const claimPairingCore = mutation({
	args: { code: v.string() },
	returns: v.union(
		v.null(),
		v.object({
			coreId: v.string(),
			connectionUrl: v.optional(v.string()),
			metadata: v.optional(pairingMetadataValidator),
			realtimeCredential: v.string(),
			syncCredential: v.string(),
		}),
	),
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)
		const code = normalizePairingCode(args.code)
		if (!code) return null
		const now = Date.now()
		await removeExpiredPairingCores(ctx, now)
		const pairing = await ctx.db
			.query('pairingCores')
			.withIndex('by_code', (index) => index.eq('code', code))
			.unique()
		if (!pairing || pairing.status !== 'waiting' || pairing.expiresAt <= now) return null
		const existingCore = await ctx.db
			.query('coreList')
			.withIndex('by_core_id', (index) => index.eq('coreId', pairing.coreId))
			.unique()
		if (existingCore && existingCore.ownerUserId !== userId)
			throw new Error('Core already belongs to another user')
		const syncCredential = createSyncCredential()
		await ctx.db.patch(pairing._id, {
			status: 'claimed',
			ownerUserId: userId,
			claimedAt: now,
			realtimeCredentialHash: await hashCredential(syncCredential),
			realtimeCredentialIssuedAt: now,
		})
		return {
			coreId: pairing.coreId,
			connectionUrl: pairing.connectionUrl,
			metadata: pairing.metadata,
			// Kept for compatibility with already-migrated Core builds; this authenticates
			// projection sync and setup verification and is not a Core-presence credential.
			realtimeCredential: syncCredential,
			syncCredential,
		}
	},
})

export const finalizePairingCore = mutation({
	args: { code: v.string(), coreId: v.string(), connectionUrl: v.optional(v.string()) },
	returns: v.object({ coreId: v.string() }),
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)
		const code = normalizePairingCode(args.code)
		if (!code) throw new Error('invalid pairing code format')
		const now = Date.now()
		await removeExpiredPairingCores(ctx, now)
		const pairing = await ctx.db
			.query('pairingCores')
			.withIndex('by_code', (index) => index.eq('code', code))
			.unique()
		if (
			!pairing ||
			pairing.status !== 'claimed' ||
			pairing.coreId !== args.coreId ||
			pairing.ownerUserId !== userId ||
			pairing.expiresAt <= now
		)
			throw new Error('pairing claim not found')
		const existing = await ctx.db
			.query('coreList')
			.withIndex('by_core_id', (index) => index.eq('coreId', pairing.coreId))
			.unique()
		if (existing && existing.ownerUserId !== userId)
			throw new Error('Core already belongs to another user')
		const value = {
			ownerUserId: userId,
			linkState: 'linked' as const,
			connectionUrl: args.connectionUrl ?? pairing.connectionUrl,
			lastSeenAt: now,
			projectionRevision: existing?.projectionRevision ?? 0,
			syncedAt: now,
			syncCredentialHash: pairing.realtimeCredentialHash,
		}
		if (existing) await ctx.db.patch(existing._id, value)
		else await ctx.db.insert('coreList', { ...value, coreId: pairing.coreId, createdAt: now })
		const ownerLink = await ctx.db
			.query('coreMemberLinks')
			.withIndex('by_core_user', (index) => index.eq('coreId', pairing.coreId).eq('userId', userId))
			.unique()
		const linkValue = { coreId: pairing.coreId, userId, isOwner: true, syncedAt: now }
		if (ownerLink) await ctx.db.patch(ownerLink._id, linkValue)
		else await ctx.db.insert('coreMemberLinks', linkValue)
		await ctx.db.delete(pairing._id)
		return { coreId: pairing.coreId }
	},
})

export const releasePairingCore = mutation({
	args: { code: v.string(), coreId: v.string() },
	returns: v.null(),
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)
		const code = normalizePairingCode(args.code)
		if (!code) return null
		const pairing = await ctx.db
			.query('pairingCores')
			.withIndex('by_code', (index) => index.eq('code', code))
			.unique()
		if (
			pairing?.status === 'claimed' &&
			pairing.coreId === args.coreId &&
			pairing.ownerUserId === userId
		) {
			await ctx.db.patch(pairing._id, {
				status: 'waiting',
				ownerUserId: undefined,
				claimedAt: undefined,
				realtimeCredentialHash: undefined,
				realtimeCredentialIssuedAt: undefined,
			})
		}
		return null
	},
})

function normalizePairingCode(code: string): string | null {
	const normalized = code
		.trim()
		.replace(/[^a-z0-9]/gi, '')
		.toLowerCase()
	return /^[a-hj-np-z2-9]{8}$/.test(normalized) ? normalized : null
}

function validId(value: unknown): value is string {
	return (
		typeof value === 'string' &&
		value.length > 0 &&
		value.length <= 256 &&
		!/[\u0000-\u001f]/.test(value)
	)
}

function validOptionalString(value: unknown, max: number): value is string {
	return typeof value === 'string' && value.length <= max && !/[\u0000-\u001f]/.test(value)
}

function clampTtl(value: number | undefined): number {
	if (value === undefined || !Number.isFinite(value)) return PAIRING_MAX_TTL_MS
	return Math.max(PAIRING_MIN_TTL_MS, Math.min(PAIRING_MAX_TTL_MS, Math.floor(value)))
}

function normalizePairingMetadata(value: { bindHost?: string; port?: number } | undefined) {
	if (!value) return undefined
	const metadata: { bindHost?: string; port?: number } = {}
	if (value.bindHost !== undefined) {
		if (!validOptionalString(value.bindHost, MAX_BIND_HOST_LENGTH))
			throw new Error('invalid bind host')
		metadata.bindHost = value.bindHost
	}
	if (value.port !== undefined) {
		if (!Number.isInteger(value.port) || value.port < 1 || value.port > 65_535)
			throw new Error('invalid port')
		metadata.port = value.port
	}
	return Object.keys(metadata).length ? metadata : undefined
}

function createSyncCredential(): string {
	const bytes = crypto.getRandomValues(new Uint8Array(32))
	return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')
}

async function hashCredential(credential: string): Promise<string> {
	const digest = new Uint8Array(
		await crypto.subtle.digest('SHA-256', new TextEncoder().encode(credential)),
	)
	return btoa(String.fromCharCode(...digest))
		.replace(/\+/g, '-')
		.replace(/\//g, '_')
		.replace(/=+$/g, '')
}

async function removeExpiredPairingCores(ctx: MutationCtx, now: number) {
	const expired = await ctx.db
		.query('pairingCores')
		.withIndex('by_expires_at', (index) => index.lte('expiresAt', now))
		.take(10)
	await Promise.all(expired.map((pairing) => ctx.db.delete(pairing._id)))
}
