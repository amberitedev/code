import { getAuthUserId } from '@convex-dev/auth/server'
import { v } from 'convex/values'
import { action, internalMutation, internalQuery, mutation, query } from './_generated/server'
import { internal } from './_generated/api'
import type { Doc, Id } from './_generated/dataModel'
import type { ActionCtx } from './_generated/server'

type ModrinthUser = {
	id: string
	username: string
	avatar_url?: string | null
}

type PublicLinkedAccount = ReturnType<typeof redactedAccount>
type TokenMaterial = {
	_id: Id<'linkedModrinthAccounts'>
	encryptedAccessToken: string
	expiresAt?: number
	public: PublicLinkedAccount
}

const linkedAccountValidator = v.object({
	id: v.id('linkedModrinthAccounts'),
	userId: v.id('users'),
	modrinthUserId: v.string(),
	username: v.string(),
	avatar_url: v.union(v.string(), v.null()),
	scopes: v.array(v.string()),
	expiresAt: v.union(v.number(), v.null()),
	status: v.union(v.literal('active'), v.literal('needs_reconnect'), v.literal('revoked')),
	needsReconnect: v.boolean(),
	reconnectReason: v.union(v.string(), v.null()),
	linkedAt: v.number(),
	updatedAt: v.number(),
})

export const current = query({
	args: {},
	returns: v.union(linkedAccountValidator, v.null()),
	handler: async (ctx) => {
		const userId = await getAuthUserId(ctx)
		if (userId === null) throw new Error('not authenticated')
		const account = await ctx.db
			.query('linkedModrinthAccounts')
			.withIndex('by_user', (q) => q.eq('userId', userId))
			.first()
		return account ? redactedAccount(account) : null
	},
})

export const disconnectCurrent = mutation({
	args: {},
	returns: v.null(),
	handler: async (ctx) => {
		const userId = await getAuthUserId(ctx)
		if (userId === null) throw new Error('not authenticated')
		const accounts = await ctx.db
			.query('linkedModrinthAccounts')
			.withIndex('by_user', (q) => q.eq('userId', userId))
			.collect()
		for (const account of accounts) await ctx.db.delete(account._id)
		return null
	},
})

export const storeCurrentOAuthTokens = action({
	args: {
		accessToken: v.string(),
		refreshToken: v.optional(v.string()),
		scopes: v.array(v.string()),
		expiresAt: v.optional(v.number()),
	},
	returns: v.object({
		modrinthUserId: v.string(),
		username: v.string(),
		avatar_url: v.union(v.string(), v.null()),
		scopes: v.array(v.string()),
		expiresAt: v.union(v.number(), v.null()),
		status: v.literal('active'),
		needsReconnect: v.literal(false),
		linkedAt: v.number(),
		updatedAt: v.number(),
	}),
	handler: async (ctx, args) => {
		const userId = await requireActionUserId(ctx)
		const modrinthUser = await fetchModrinthUser(args.accessToken)
		if (!modrinthUser) throw new Error('Modrinth token is invalid')
		const now = Date.now()
		await ctx.runMutation(internal.modrinth.upsertLinkedAccount, {
			userId,
			modrinthUserId: modrinthUser.id,
			username: modrinthUser.username,
			avatarUrl: modrinthUser.avatar_url ?? undefined,
			scopes: [...new Set(args.scopes)].sort(),
			encryptedAccessToken: await encryptToken(args.accessToken),
			encryptedRefreshToken: args.refreshToken ? await encryptToken(args.refreshToken) : undefined,
			expiresAt: args.expiresAt,
			linkedAt: now,
			updatedAt: now,
		})
		return {
			modrinthUserId: modrinthUser.id,
			username: modrinthUser.username,
			avatar_url: modrinthUser.avatar_url ?? null,
			scopes: [...new Set(args.scopes)].sort(),
			expiresAt: args.expiresAt ?? null,
			status: 'active' as const,
			needsReconnect: false as const,
			linkedAt: now,
			updatedAt: now,
		}
	},
})

export const refreshCurrentStatus = action({
	args: {},
	returns: v.union(linkedAccountValidator, v.null()),
	handler: async (ctx): Promise<PublicLinkedAccount | null> => {
		const userId = await requireActionUserId(ctx)
		const account: TokenMaterial | null = await ctx.runQuery(internal.modrinth.tokenMaterial, {
			userId,
		})
		if (!account) return null
		if (account.expiresAt && account.expiresAt <= Date.now()) {
			await ctx.runMutation(internal.modrinth.markNeedsReconnect, {
				accountId: account._id,
				reason: 'expired',
			})
			return { ...account.public, status: 'needs_reconnect' as const, needsReconnect: true }
		}
		const modrinthUser = await fetchModrinthUser(await decryptToken(account.encryptedAccessToken))
		if (!modrinthUser) {
			await ctx.runMutation(internal.modrinth.markNeedsReconnect, {
				accountId: account._id,
				reason: 'revoked',
			})
			return { ...account.public, status: 'needs_reconnect' as const, needsReconnect: true }
		}
		await ctx.runMutation(internal.modrinth.updateProfileSnapshot, {
			accountId: account._id,
			username: modrinthUser.username,
			avatarUrl: modrinthUser.avatar_url ?? undefined,
		})
		return {
			...account.public,
			username: modrinthUser.username,
			avatar_url: modrinthUser.avatar_url ?? null,
			status: 'active' as const,
			needsReconnect: false,
		}
	},
})

export const tokenMaterial = internalQuery({
	args: { userId: v.id('users') },
	returns: v.union(
		v.object({
			_id: v.id('linkedModrinthAccounts'),
			encryptedAccessToken: v.string(),
			expiresAt: v.optional(v.number()),
			public: linkedAccountValidator,
		}),
		v.null(),
	),
	handler: async (ctx, args) => {
		const account = await ctx.db
			.query('linkedModrinthAccounts')
			.withIndex('by_user', (q) => q.eq('userId', args.userId))
			.first()
		if (!account) return null
		return {
			_id: account._id,
			encryptedAccessToken: account.encryptedAccessToken,
			expiresAt: account.expiresAt,
			public: redactedAccount(account),
		}
	},
})

export const upsertLinkedAccount = internalMutation({
	args: {
		userId: v.id('users'),
		modrinthUserId: v.string(),
		username: v.string(),
		avatarUrl: v.optional(v.string()),
		scopes: v.array(v.string()),
		encryptedAccessToken: v.string(),
		encryptedRefreshToken: v.optional(v.string()),
		expiresAt: v.optional(v.number()),
		linkedAt: v.number(),
		updatedAt: v.number(),
	},
	returns: v.null(),
	handler: async (ctx, args) => {
		const existing = await ctx.db
			.query('linkedModrinthAccounts')
			.withIndex('by_user', (q) => q.eq('userId', args.userId))
			.first()
		const value = {
			modrinthUserId: args.modrinthUserId,
			username: args.username,
			avatarUrl: args.avatarUrl,
			scopes: args.scopes,
			encryptedAccessToken: args.encryptedAccessToken,
			encryptedRefreshToken: args.encryptedRefreshToken,
			expiresAt: args.expiresAt,
			status: 'active' as const,
			needsReconnect: false,
			reconnectReason: undefined,
			updatedAt: args.updatedAt,
		}
		if (existing) await ctx.db.patch(existing._id, value)
		else
			await ctx.db.insert('linkedModrinthAccounts', {
				...value,
				userId: args.userId,
				linkedAt: args.linkedAt,
			})
		return null
	},
})

export const markNeedsReconnect = internalMutation({
	args: {
		accountId: v.id('linkedModrinthAccounts'),
		reason: v.string(),
	},
	returns: v.null(),
	handler: async (ctx, args) => {
		await ctx.db.patch(args.accountId, {
			status: 'needs_reconnect',
			needsReconnect: true,
			reconnectReason: args.reason,
			updatedAt: Date.now(),
		})
		return null
	},
})

export const updateProfileSnapshot = internalMutation({
	args: {
		accountId: v.id('linkedModrinthAccounts'),
		username: v.string(),
		avatarUrl: v.optional(v.string()),
	},
	returns: v.null(),
	handler: async (ctx, args) => {
		await ctx.db.patch(args.accountId, {
			username: args.username,
			avatarUrl: args.avatarUrl,
			status: 'active',
			needsReconnect: false,
			reconnectReason: undefined,
			updatedAt: Date.now(),
		})
		return null
	},
})

async function requireActionUserId(ctx: ActionCtx): Promise<Id<'users'>> {
	const userId = await getAuthUserId(ctx)
	if (userId === null) throw new Error('not authenticated')
	return userId
}

async function fetchModrinthUser(accessToken: string): Promise<ModrinthUser | null> {
	const response = await fetch('https://api.modrinth.com/v2/user', {
		headers: {
			Authorization: `Bearer ${accessToken}`,
			'User-Agent': 'Amberite/0.1 (profiles backend)',
		},
	})
	if (!response.ok) return null
	return (await response.json()) as ModrinthUser
}

function redactedAccount(account: Doc<'linkedModrinthAccounts'>) {
	return {
		id: account._id,
		userId: account.userId,
		modrinthUserId: account.modrinthUserId,
		username: account.username,
		avatar_url: account.avatarUrl ?? null,
		scopes: account.scopes,
		expiresAt: account.expiresAt ?? null,
		status: account.status,
		needsReconnect: account.status !== 'active' || account.needsReconnect === true,
		reconnectReason: account.reconnectReason ?? null,
		linkedAt: account.linkedAt,
		updatedAt: account.updatedAt,
	}
}

async function encryptToken(token: string): Promise<string> {
	const iv = crypto.getRandomValues(new Uint8Array(12))
	const encrypted = new Uint8Array(
		await crypto.subtle.encrypt(
			{ name: 'AES-GCM', iv },
			await encryptionKey(),
			new TextEncoder().encode(token),
		),
	)
	const bytes = new Uint8Array(iv.length + encrypted.length)
	bytes.set(iv)
	bytes.set(encrypted, iv.length)
	return base64Url(bytes)
}

async function decryptToken(value: string): Promise<string> {
	const bytes = fromBase64Url(value)
	const iv = bytes.slice(0, 12)
	const encrypted = bytes.slice(12)
	const decrypted = await crypto.subtle.decrypt(
		{ name: 'AES-GCM', iv },
		await encryptionKey(),
		encrypted,
	)
	return new TextDecoder().decode(decrypted)
}

async function encryptionKey(): Promise<CryptoKey> {
	const secret = process.env.MODRINTH_TOKEN_ENCRYPTION_KEY
	if (!secret) throw new Error('MODRINTH_TOKEN_ENCRYPTION_KEY must be configured')
	const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(secret))
	return crypto.subtle.importKey('raw', digest, { name: 'AES-GCM' }, false, ['encrypt', 'decrypt'])
}

function base64Url(bytes: Uint8Array): string {
	let value = ''
	for (const byte of bytes) value += String.fromCharCode(byte)
	return btoa(value).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '')
}

function fromBase64Url(value: string): Uint8Array {
	const base64 =
		value.replace(/-/g, '+').replace(/_/g, '/') + '='.repeat((4 - (value.length % 4)) % 4)
	return Uint8Array.from(atob(base64), (character) => character.charCodeAt(0))
}
