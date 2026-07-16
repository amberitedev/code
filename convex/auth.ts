import { ConvexCredentials } from '@convex-dev/auth/providers/ConvexCredentials'
import { convexAuth, createAccount, getAuthUserId, retrieveAccount } from '@convex-dev/auth/server'
import { v } from 'convex/values'
import { internal } from './_generated/api'
import { internalMutation, internalQuery, mutation, query } from './_generated/server'
import { currentAccountFields, publicUser, requireUserId } from './_socialRules'

interface MinecraftProfile {
	id: string
	name: string
}

const MINECRAFT_TOKEN_PROVIDER_ID = 'minecraft-token'
const DEV_ACCOUNT_PROVIDER_ID = 'amberite-dev-account'
const INVALID_ACCOUNT_ID = 'InvalidAccountId'
const USERNAME_PATTERN = /^[a-zA-Z0-9_]{3,24}$/

function normalizeUuid(raw: string): string {
	const stripped = raw.replace(/-/g, '')
	if (stripped.length !== 32) return raw
	return [
		stripped.slice(0, 8),
		stripped.slice(8, 12),
		stripped.slice(12, 16),
		stripped.slice(16, 20),
		stripped.slice(20, 32),
	].join('-')
}

async function verifyMinecraftAccessToken(token: string): Promise<MinecraftProfile | null> {
	const response = await fetch('https://api.minecraftservices.com/minecraft/profile', {
		headers: { Authorization: `Bearer ${token}` },
	})
	if (!response.ok) return null
	return (await response.json()) as MinecraftProfile
}

function accountProfile(gamertag: string, accountId: string) {
	return {
		amberiteUserId: accountId,
		friendCode: createFriendCode(),
		displayName: gamertag,
		username: gamertag,
		normalizedUsername: gamertag.toLowerCase(),
		onboardedAt: Date.now(),
	}
}

function createFriendCode(): string {
	const bytes = crypto.getRandomValues(new Uint8Array(5))
	return `AMB-${Array.from(bytes, (byte) => byte.toString(36).padStart(2, '0'))
		.join('')
		.slice(0, 8)
		.toUpperCase()}`
}

export const { auth, signIn, signOut, store, isAuthenticated } = convexAuth({
	providers: [
		ConvexCredentials({
			id: MINECRAFT_TOKEN_PROVIDER_ID,
			authorize: async (credentials, ctx) => {
				const minecraftAccessToken = credentials.minecraftAccessToken as string | undefined
				if (!minecraftAccessToken) return null

				const profile = await verifyMinecraftAccessToken(minecraftAccessToken)
				if (!profile) return null

				const minecraftUuid = normalizeUuid(profile.id)
				const gamertag = profile.name
				const accountId = `minecraft:${minecraftUuid}`

				const existing = await retrieveAccount(ctx, {
					provider: MINECRAFT_TOKEN_PROVIDER_ID,
					account: { id: accountId },
				}).catch((error) => {
					if (error instanceof Error && error.message === INVALID_ACCOUNT_ID) return null
					throw error
				})
				if (existing) {
					const amberiteUserId = existing.user.amberiteUserId ?? accountId
					await ctx.runMutation(internal.auth.ensureLinkedMinecraftAccount, {
						userId: existing.user._id,
						amberiteUserId,
						accountId,
						gamertag,
						minecraftUuid,
					})
					return { userId: existing.user._id }
				}

				const accountDocument = accountProfile(gamertag, accountId)
				const { user } = await createAccount(ctx, {
					provider: MINECRAFT_TOKEN_PROVIDER_ID,
					account: { id: accountId },
					profile: accountDocument,
				})
				await ctx.runMutation(internal.auth.ensureLinkedMinecraftAccount, {
					userId: user._id,
					amberiteUserId: accountDocument.amberiteUserId,
					accountId,
					gamertag,
					minecraftUuid,
				})

				return { userId: user._id }
			},
		}),
		...(process.env.AMBERITE_DEV_MODE === 'true'
			? [
					ConvexCredentials({
						id: DEV_ACCOUNT_PROVIDER_ID,
						authorize: async (credentials, ctx) => {
							if (process.env.AMBERITE_DEV_MODE !== 'true') {
								throw new Error('Amberite development accounts are disabled')
							}
							const username = credentials.username
							if (typeof username !== 'string' || !USERNAME_PATTERN.test(username)) {
								throw new Error('Invalid Amberite development account username')
							}
							const userId = await ctx.runQuery(internal.auth.devAccountUserId, {
								username,
							})
							if (!userId) throw new Error(`Amberite dev user ${username} does not exist`)
							return { userId }
						},
					}),
				]
			: []),
	],
})

export const devAccountUserId = internalQuery({
	args: { username: v.string() },
	handler: async (ctx, args) => {
		if (process.env.AMBERITE_DEV_MODE !== 'true') {
			throw new Error('Amberite development accounts are disabled')
		}
		const user = await ctx.db
			.query('users')
			.withIndex('by_normalized_username', (q) =>
				q.eq('normalizedUsername', args.username.toLowerCase()),
			)
			.unique()
		return user && !user.deletedAt ? user._id : null
	},
})

export const usernameAvailable = internalQuery({
	args: { username: v.string() },
	handler: async (ctx, args) => {
		const username = normalizeUsername(args.username)
		const existing = await ctx.db
			.query('users')
			.withIndex('by_normalized_username', (q) =>
				q.eq('normalizedUsername', username.toLowerCase()),
			)
			.first()
		return existing === null
	},
})

export const deleteCurrentAccount = mutation({
	args: {},
	handler: async (ctx) => {
		const userId = await requireUserId(ctx)
		await ctx.db.patch(userId, {
			deletedAt: Date.now(),
			deletedReason: 'user requested deletion',
		})
		return { ok: true }
	},
})

function normalizeUsername(value: string): string {
	const username = value.trim()
	if (!USERNAME_PATTERN.test(username)) {
		throw new Error('username must be 3-24 letters, numbers, or underscores')
	}
	return username
}

export const ensureLinkedMinecraftAccount = internalMutation({
	args: {
		userId: v.id('users'),
		amberiteUserId: v.string(),
		accountId: v.string(),
		gamertag: v.string(),
		minecraftUuid: v.string(),
	},
	handler: async (ctx, args) => {
		const user = await ctx.db.get(args.userId)
		if (!user) throw new Error('user does not exist')
		if (!user.amberiteUserId) {
			await ctx.db.patch(args.userId, { amberiteUserId: args.amberiteUserId })
		}

		const existing = await ctx.db
			.query('linkedMicrosoftAccounts')
			.withIndex('by_amberite_user', (q) => q.eq('amberiteUserId', args.amberiteUserId))
			.first()
		if (existing) {
			await ctx.db.patch(existing._id, {
				microsoftAccountId: args.accountId,
				gamertag: args.gamertag,
				minecraftUuid: args.minecraftUuid,
			})
			return
		}
		await ctx.db.insert('linkedMicrosoftAccounts', {
			amberiteUserId: args.amberiteUserId,
			microsoftAccountId: args.accountId,
			gamertag: args.gamertag,
			minecraftUuid: args.minecraftUuid,
			linkedAt: Date.now(),
		})
	},
})

export const currentUser = query({
	args: {},
	handler: async (ctx) => {
		const userId = await getAuthUserId(ctx)
		if (userId === null) return null
		const user = await ctx.db.get(userId)
		return user
			? publicUser({ ...user, _id: userId }, true, await currentAccountFields(ctx, userId))
			: null
	},
})

export const setUsername = mutation({
	args: {
		username: v.string(),
		displayName: v.optional(v.string()),
	},
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)

		const username = args.username.trim()
		if (!/^[a-zA-Z0-9_]{3,24}$/.test(username)) {
			throw new Error('username must be 3-24 letters, numbers, or underscores')
		}

		const normalizedUsername = username.toLowerCase()
		const existing = await ctx.db
			.query('users')
			.withIndex('by_normalized_username', (q) => q.eq('normalizedUsername', normalizedUsername))
			.unique()
		if (existing && existing._id !== userId) throw new Error('username is already taken')

		const user = await ctx.db.get(userId)
		const now = Date.now()
		await ctx.db.patch(userId, {
			amberiteUserId: user?.amberiteUserId ?? crypto.randomUUID(),
			username,
			normalizedUsername,
			displayName: args.displayName?.trim() || username,
			onboardedAt: user?.onboardedAt ?? now,
		})

		return { userId, username }
	},
})
