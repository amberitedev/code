import { ConvexCredentials } from '@convex-dev/auth/providers/ConvexCredentials'
import { convexAuth, createAccount, getAuthUserId, retrieveAccount } from '@convex-dev/auth/server'
import { v } from 'convex/values'
import { internal } from './_generated/api'
import type { Id } from './_generated/dataModel'
import { internalMutation, internalQuery, mutation, query } from './_generated/server'
import { currentAccountFields, publicUser, requireUserId } from './_socialRules'
import {
	AMBERITE_SESSION_POLICY,
	normalizeMinecraftHandle,
	normalizeMinecraftUuid,
	shouldSyncDefaultMinecraftDisplayName,
} from './minecraftIdentity'

interface MinecraftProfile {
	id: string
	name: string
}

const MINECRAFT_TOKEN_PROVIDER_ID = 'minecraft-token'
const DEV_ACCOUNT_PROVIDER_ID = 'amberite-dev-account'
const INVALID_ACCOUNT_ID = 'InvalidAccountId'
const USERNAME_PATTERN = /^[a-zA-Z0-9_]{3,24}$/

async function verifyMinecraftAccessToken(token: string): Promise<MinecraftProfile> {
	let response: Response
	try {
		response = await fetch('https://api.minecraftservices.com/minecraft/profile', {
			headers: { Authorization: `Bearer ${token}` },
		})
	} catch {
		throw new Error('MinecraftProviderUnavailable')
	}
	if (response.status === 404) throw new Error('MinecraftJavaProfileMissing')
	if (response.status === 429) throw new Error('MinecraftProviderThrottled')
	if (response.status === 401 || response.status === 403)
		throw new Error('MinecraftAccessTokenInvalid')
	if (!response.ok) throw new Error('MinecraftProviderUnavailable')
	const profile = (await response.json()) as MinecraftProfile
	return {
		id: normalizeMinecraftUuid(profile.id),
		name: normalizeMinecraftHandle(profile.name),
	}
}

function accountProfile(handle: string, accountId: string, minecraftUuid: string) {
	const now = Date.now()
	return {
		amberiteUserId: accountId,
		friendCode: createFriendCode(),
		displayName: handle,
		name: handle,
		username: handle,
		normalizedUsername: handle.toLowerCase(),
		minecraftUuid,
		verifiedMinecraftHandle: handle,
		normalizedVerifiedMinecraftHandle: handle.toLowerCase(),
		minecraftVerifiedAt: now,
		minecraftLastVerifiedAt: now,
		onboardedAt: now,
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
	session: {
		totalDurationMs: AMBERITE_SESSION_POLICY.totalDurationMs,
		inactiveDurationMs: AMBERITE_SESSION_POLICY.inactiveDurationMs,
	},
	jwt: { durationMs: AMBERITE_SESSION_POLICY.jwtDurationMs },
	providers: [
		ConvexCredentials({
			id: MINECRAFT_TOKEN_PROVIDER_ID,
			authorize: async (credentials, ctx) => {
				const minecraftAccessToken = credentials.minecraftAccessToken
				if (typeof minecraftAccessToken !== 'string' || !minecraftAccessToken) return null
				const profile = await verifyMinecraftAccessToken(minecraftAccessToken)
				const minecraftUuid = profile.id
				const expected = credentials.expectedMinecraftUuid
				if (typeof expected === 'string' && normalizeMinecraftUuid(expected) !== minecraftUuid) {
					throw new Error('MinecraftUuidMismatch')
				}
				const accountId = `minecraft:${minecraftUuid}`
				const identity = await ctx.runQuery(internal.auth.identityForMinecraftUuid, {
					minecraftUuid,
				})
				if (identity.ambiguous) throw new Error('AmbiguousMinecraftIdentity')

				const existing = await retrieveAccount(ctx, {
					provider: MINECRAFT_TOKEN_PROVIDER_ID,
					account: { id: accountId },
				}).catch((error) => {
					if (error instanceof Error && error.message === INVALID_ACCOUNT_ID) return null
					throw error
				})
				if (existing) {
					if (identity.userId && identity.userId !== existing.user._id)
						throw new Error('AmbiguousMinecraftIdentity')
					await ctx.runMutation(internal.auth.synchronizeMinecraftIdentity, {
						userId: existing.user._id,
						amberiteUserId: existing.user.amberiteUserId ?? accountId,
						accountId,
						handle: profile.name,
						minecraftUuid,
					})
					return { userId: existing.user._id }
				}

				if (identity.userId) throw new Error('MinecraftIdentityMigrationRequired')
				const document = accountProfile(profile.name, accountId, minecraftUuid)
				const { user } = await createAccount(ctx, {
					provider: MINECRAFT_TOKEN_PROVIDER_ID,
					account: { id: accountId },
					profile: document,
				})
				await ctx.runMutation(internal.auth.synchronizeMinecraftIdentity, {
					userId: user._id,
					amberiteUserId: document.amberiteUserId,
					accountId,
					handle: profile.name,
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
							if (process.env.AMBERITE_DEV_MODE !== 'true')
								throw new Error('Amberite development accounts are disabled')
							const username = credentials.username
							if (typeof username !== 'string' || !USERNAME_PATTERN.test(username))
								throw new Error('Invalid Amberite development account username')
							const userId = await ctx.runQuery(internal.auth.devAccountUserId, { username })
							if (!userId) throw new Error(`Amberite dev user ${username} does not exist`)
							return { userId }
						},
					}),
				]
			: []),
	],
})

export const identityForMinecraftUuid = internalQuery({
	args: { minecraftUuid: v.string() },
	returns: v.object({ userId: v.union(v.id('users'), v.null()), ambiguous: v.boolean() }),
	handler: async (ctx, args) => {
		const minecraftUuid = normalizeMinecraftUuid(args.minecraftUuid)
		const [users, links] = await Promise.all([
			ctx.db
				.query('users')
				.withIndex('by_minecraft_uuid', (q) => q.eq('minecraftUuid', minecraftUuid))
				.collect(),
			ctx.db
				.query('linkedMicrosoftAccounts')
				.withIndex('by_minecraft_uuid', (q) => q.eq('minecraftUuid', minecraftUuid))
				.collect(),
		])
		const usersById = new Map<string, Id<'users'>>()
		for (const user of users) {
			if (!user.deletedAt) usersById.set(user._id.toString(), user._id)
		}
		for (const link of links) {
			const user = await ctx.db
				.query('users')
				.withIndex('by_amberite_user_id', (q) => q.eq('amberiteUserId', link.amberiteUserId))
				.first()
			if (user && !user.deletedAt) usersById.set(user._id.toString(), user._id)
		}
		return {
			userId: usersById.size === 1 ? [...usersById.values()][0] : null,
			ambiguous: usersById.size > 1 || links.length > 1,
		}
	},
})

export const synchronizeMinecraftIdentity = internalMutation({
	args: {
		userId: v.id('users'),
		amberiteUserId: v.string(),
		accountId: v.string(),
		handle: v.string(),
		minecraftUuid: v.string(),
	},
	returns: v.null(),
	handler: async (ctx, args) => {
		const user = await ctx.db.get(args.userId)
		if (!user) throw new Error('user does not exist')
		const minecraftUuid = normalizeMinecraftUuid(args.minecraftUuid)
		const handle = normalizeMinecraftHandle(args.handle)
		const duplicateUsers = await ctx.db
			.query('users')
			.withIndex('by_minecraft_uuid', (q) => q.eq('minecraftUuid', minecraftUuid))
			.collect()
		if (duplicateUsers.some((candidate) => candidate._id !== args.userId && !candidate.deletedAt))
			throw new Error('AmbiguousMinecraftIdentity')
		const handleOwner = await ctx.db
			.query('users')
			.withIndex('by_verified_minecraft_handle', (q) =>
				q.eq('normalizedVerifiedMinecraftHandle', handle.toLowerCase()),
			)
			.first()
		if (handleOwner && handleOwner._id !== args.userId && !handleOwner.deletedAt)
			throw new Error('MinecraftHandleConflict')

		const previousHandle = user.verifiedMinecraftHandle ?? user.username
		const syncDisplayName = shouldSyncDefaultMinecraftDisplayName(
			user.displayName,
			user.name,
			previousHandle,
		)
		const now = Date.now()
		await ctx.db.patch(args.userId, {
			amberiteUserId: user.amberiteUserId ?? args.amberiteUserId,
			minecraftUuid,
			verifiedMinecraftHandle: handle,
			normalizedVerifiedMinecraftHandle: handle.toLowerCase(),
			username: handle,
			normalizedUsername: handle.toLowerCase(),
			minecraftVerifiedAt: user.minecraftVerifiedAt ?? now,
			minecraftLastVerifiedAt: now,
			...(syncDisplayName ? { displayName: handle, name: handle } : {}),
		})

		const links = await ctx.db
			.query('linkedMicrosoftAccounts')
			.withIndex('by_amberite_user', (q) => q.eq('amberiteUserId', args.amberiteUserId))
			.collect()
		if (links.length > 1) throw new Error('AmbiguousMinecraftIdentity')
		const value = {
			microsoftAccountId: args.accountId,
			gamertag: handle,
			minecraftUuid,
			verifiedAt: links[0]?.verifiedAt ?? now,
			lastVerifiedAt: now,
		}
		if (links[0]) await ctx.db.patch(links[0]._id, value)
		else
			await ctx.db.insert('linkedMicrosoftAccounts', {
				amberiteUserId: args.amberiteUserId,
				linkedAt: now,
				...value,
			})
		return null
	},
})

export const devAccountUserId = internalQuery({
	args: { username: v.string() },
	returns: v.union(v.id('users'), v.null()),
	handler: async (ctx, args) => {
		if (process.env.AMBERITE_DEV_MODE !== 'true')
			throw new Error('Amberite development accounts are disabled')
		const user = await ctx.db
			.query('users')
			.withIndex('by_normalized_username', (q) =>
				q.eq('normalizedUsername', args.username.toLowerCase()),
			)
			.unique()
		return user && !user.deletedAt ? user._id : null
	},
})

export const deleteCurrentAccount = mutation({
	args: {},
	returns: v.object({ ok: v.boolean() }),
	handler: async (ctx) => {
		const userId = await requireUserId(ctx)
		await ctx.db.patch(userId, { deletedAt: Date.now(), deletedReason: 'user requested deletion' })
		return { ok: true }
	},
})

export const currentUser = query({
	args: {},
	returns: v.any(),
	handler: async (ctx) => {
		const userId = await getAuthUserId(ctx)
		if (userId === null) return null
		const user = await ctx.db.get(userId)
		return user
			? publicUser({ ...user, _id: userId }, true, await currentAccountFields(ctx, userId))
			: null
	},
})
