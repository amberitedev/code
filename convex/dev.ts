/**
 * Dev-only utilities for exercising the social/friend-group system before real
 * Microsoft auth is wired. Every function here is guarded by AMBERITE_DEV_MODE
 * and is structurally inert in production (the flag is never set there).
 *
 * Functions: seedDevUsers, listDevUsers, devState, resetSocial.
 */
import { v } from 'convex/values'
import { mutation, query } from './_generated/server'
import type { MutationCtx } from './_generated/server'
import { isDevMode, publicUser } from './_socialRules'

function assertDev() {
	if (!isDevMode()) throw new Error('dev functions are disabled (AMBERITE_DEV_MODE not set)')
}

/** The four seeded "real" Amberite users used for manual + automated testing. */
const SEED_USERS = [
	{ username: 'amber', displayName: 'Amber (you)', gamertag: 'AmberCraft', minecraftUuid: '11111111-1111-4111-8111-111111111111' },
	{ username: 'birch', displayName: 'Birch', gamertag: 'BirchBuilder', minecraftUuid: '22222222-2222-4222-8222-222222222222' },
	{ username: 'cedar', displayName: 'Cedar', gamertag: 'CedarSurvival', minecraftUuid: '33333333-3333-4333-8333-333333333333' },
	{ username: 'dahlia', displayName: 'Dahlia', gamertag: 'DahliaMines', minecraftUuid: '44444444-4444-4444-8444-444444444444' },
] as const

/** Ensure a seeded user has a linked Minecraft account so auto-whitelist resolves. */
async function ensureLinkedAccount(
	ctx: MutationCtx,
	amberiteUserId: string,
	seed: { username: string; gamertag: string; minecraftUuid: string },
): Promise<void> {
	const existing = await ctx.db
		.query('linkedMicrosoftAccounts')
		.withIndex('by_amberite_user', (q) => q.eq('amberiteUserId', amberiteUserId))
		.first()
	if (existing) return
	await ctx.db.insert('linkedMicrosoftAccounts', {
		amberiteUserId,
		microsoftAccountId: `dev-ms-${seed.username}`,
		gamertag: seed.gamertag,
		minecraftUuid: seed.minecraftUuid,
		linkedAt: Date.now(),
	})
}

async function allocateFriendCode(ctx: MutationCtx): Promise<string> {
	for (let i = 0; i < 10; i++) {
		const code = `AMB-${Math.random().toString(36).slice(2, 8).toUpperCase()}`
		const existing = await ctx.db
			.query('users')
			.withIndex('by_friend_code', (q) => q.eq('friendCode', code))
			.unique()
		if (!existing) return code
	}
	throw new Error('could not allocate friend code')
}

/**
 * Create (or top up) the four seeded users. Idempotent: matches on
 * normalizedUsername, fills in any missing friendCode / amberiteUserId.
 */
export const seedDevUsers = mutation({
	args: {},
	handler: async (ctx) => {
		assertDev()
		const now = Date.now()
		const result = []
		for (const seed of SEED_USERS) {
			const normalizedUsername = seed.username.toLowerCase()
			const existing = await ctx.db
				.query('users')
				.withIndex('by_normalized_username', (q) =>
					q.eq('normalizedUsername', normalizedUsername),
				)
				.unique()
			if (existing) {
				const patch: Record<string, unknown> = {}
				if (!existing.friendCode) patch.friendCode = await allocateFriendCode(ctx)
				if (!existing.amberiteUserId) patch.amberiteUserId = crypto.randomUUID()
				if (Object.keys(patch).length > 0) await ctx.db.patch(existing._id, patch)
				const amberiteUserId = (patch.amberiteUserId as string) ?? existing.amberiteUserId
				if (amberiteUserId) await ensureLinkedAccount(ctx, amberiteUserId, seed)
				result.push({ ...publicUser({ ...existing, ...patch, _id: existing._id }), userId: existing._id })
				continue
			}
			const userId = await ctx.db.insert('users', {
				username: seed.username,
				normalizedUsername,
				displayName: seed.displayName,
				amberiteUserId: crypto.randomUUID(),
				friendCode: await allocateFriendCode(ctx),
				onboardedAt: now,
			})
			const user = await ctx.db.get(userId)
			if (user?.amberiteUserId) await ensureLinkedAccount(ctx, user.amberiteUserId, seed)
			result.push({ ...publicUser({ ...user, _id: userId }), userId })
		}
		return result
	},
})

/** List the seeded users so the desktop app can offer an "act as" picker. */
export const listDevUsers = query({
	args: {},
	handler: async (ctx) => {
		assertDev()
		const users = await Promise.all(
			SEED_USERS.map((seed) =>
				ctx.db
					.query('users')
					.withIndex('by_normalized_username', (q) =>
						q.eq('normalizedUsername', seed.username.toLowerCase()),
					)
					.unique(),
			),
		)
		return users
			.filter(Boolean)
			.map((user) => ({ ...publicUser(user), userId: user!._id }))
	},
})

/** Full snapshot of social state — handy for inspecting flows from DevTools. */
export const devState = query({
	args: {},
	handler: async (ctx) => {
		assertDev()
		const [groups, members, requests, friendships, invites, bans, cores, pairing] =
			await Promise.all([
				ctx.db.query('friendGroups').collect(),
				ctx.db.query('friendGroupMembers').collect(),
				ctx.db.query('friendRequests').collect(),
				ctx.db.query('friendships').collect(),
				ctx.db.query('friendGroupInvites').collect(),
				ctx.db.query('friendGroupBans').collect(),
				ctx.db.query('cores').collect(),
				ctx.db.query('pairingCores').collect(),
			])
		return { groups, members, requests, friendships, invites, bans, cores, pairing }
	},
})

const SOCIAL_TABLES = [
	'friendGroups',
	'friendGroupMembers',
	'friendRequests',
	'friendships',
	'blockedUsers',
	'friendGroupInvites',
	'friendGroupBans',
	'cores',
	'pairingCores',
	'syncedProfiles',
	'profileSnapshots',
	'modSyncEvents',
] as const

/** Wipe all social state (keeps user accounts) for a clean testing slate. */
export const resetSocial = mutation({
	args: { includeUsers: v.optional(v.boolean()) },
	handler: async (ctx, args) => {
		assertDev()
		for (const table of SOCIAL_TABLES) {
			const rows = await ctx.db.query(table).collect()
			for (const row of rows) await ctx.db.delete(row._id)
		}
		if (args.includeUsers) {
			const users = await ctx.db.query('users').collect()
			for (const user of users) await ctx.db.delete(user._id)
		}
		return null
	},
})
