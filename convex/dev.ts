import { v } from 'convex/values'
import { mutation, query } from './_generated/server'
import type { MutationCtx } from './_generated/server'
import { isDevMode, requireUserId } from './_socialRules'

const BASELINE_ACCOUNTS = [
	{ username: 'scenario_1', displayName: 'Owner', friendCode: 'AMB-OWNER001' },
	{ username: 'scenario_2', displayName: 'Admin', friendCode: 'AMB-ADMIN001' },
	{ username: 'scenario_3', displayName: 'Member', friendCode: 'AMB-MEMBER01' },
	{ username: 'scenario_4', displayName: 'Invited', friendCode: 'AMB-INVITE01' },
] as const
const accountIdsValidator = v.record(v.string(), v.string())

export const ensureAccounts = mutation({
	args: {},
	returns: accountIdsValidator,
	handler: async (ctx) => {
		assertDev()
		return await ensureDevAccounts(ctx, [1, 2, 3, 4])
	},
})

export const ensureScenarios = mutation({
	args: { scenarios: v.array(v.number()) },
	returns: accountIdsValidator,
	handler: async (ctx, args) => {
		assertDev()
		const scenarios = [...new Set(args.scenarios)]
		for (const scenario of scenarios) assertScenario(scenario)
		return await ensureDevAccounts(ctx, scenarios)
	},
})

export const applyBaseline = mutation({
	args: { baseline: v.literal('accounts') },
	returns: v.object({ baseline: v.literal('accounts'), accounts: accountIdsValidator }),
	handler: async (ctx) => {
		assertDev()
		await clearSocialState(ctx)
		return { baseline: 'accounts' as const, accounts: await ensureDevAccounts(ctx, [1, 2, 3, 4]) }
	},
})

export const devState = query({
	args: {},
	returns: v.object({
		users: v.array(v.id('users')),
		friendships: v.array(v.id('friendships')),
		friendRequests: v.array(v.id('friendRequests')),
		notifications: v.array(v.id('socialNotifications')),
		cores: v.array(v.id('coreList')),
	}),
	handler: async (ctx) => {
		assertDev()
		const [users, friendships, friendRequests, notifications, cores] = await Promise.all([
			ctx.db.query('users').take(100),
			ctx.db.query('friendships').take(100),
			ctx.db.query('friendRequests').take(100),
			ctx.db.query('socialNotifications').take(100),
			ctx.db.query('coreList').take(100),
		])
		return {
			users: users.map((row) => row._id),
			friendships: friendships.map((row) => row._id),
			friendRequests: friendRequests.map((row) => row._id),
			notifications: notifications.map((row) => row._id),
			cores: cores.map((row) => row._id),
		}
	},
})

export const resetSocial = mutation({
	args: {},
	returns: v.null(),
	handler: async (ctx) => {
		assertDev()
		await clearSocialState(ctx)
		return null
	},
})

/** Destructively replace the development deployment with deterministic scenario accounts. */
export const resetAll = mutation({
	args: {},
	returns: accountIdsValidator,
	handler: async (ctx) => {
		assertDev()
		await clearSocialState(ctx)
		for (const row of await ctx.db.query('deviceSessions').collect()) await ctx.db.delete(row._id)
		for (const row of await ctx.db.query('authRefreshTokens').collect())
			await ctx.db.delete(row._id)
		for (const row of await ctx.db.query('authVerificationCodes').collect())
			await ctx.db.delete(row._id)
		for (const row of await ctx.db.query('authVerifiers').collect()) await ctx.db.delete(row._id)
		for (const row of await ctx.db.query('authSessions').collect()) await ctx.db.delete(row._id)
		for (const row of await ctx.db.query('authAccounts').collect()) await ctx.db.delete(row._id)
		for (const row of await ctx.db.query('authRateLimits').collect()) await ctx.db.delete(row._id)
		for (const row of await ctx.db.query('linkedMicrosoftAccounts').collect())
			await ctx.db.delete(row._id)
		for (const row of await ctx.db.query('linkedModrinthAccounts').collect())
			await ctx.db.delete(row._id)
		for (const row of await ctx.db.query('realtimeBridgeRequests').collect())
			await ctx.db.delete(row._id)
		for (const row of await ctx.db.query('users').collect()) await ctx.db.delete(row._id)
		return await ensureDevAccounts(ctx, [1, 2, 3, 4])
	},
})

export const clearCoreLink = mutation({
	args: { coreId: v.optional(v.string()) },
	returns: v.object({ clearedCoreIds: v.array(v.string()) }),
	handler: async (ctx, args) => {
		assertDev()
		const userId = await requireUserId(ctx)
		const links = await ctx.db
			.query('coreMemberLinks')
			.withIndex('by_user', (index) => index.eq('userId', userId))
			.collect()
		const coreIds = new Set(
			links
				.filter((link) => link.isOwner && (!args.coreId || link.coreId === args.coreId))
				.map((link) => link.coreId),
		)
		const owned = await ctx.db
			.query('coreList')
			.withIndex('by_owner', (index) => index.eq('ownerUserId', userId))
			.collect()
		for (const core of owned)
			if (!args.coreId || core.coreId === args.coreId) coreIds.add(core.coreId)
		for (const coreId of coreIds) await deleteCoreLinkState(ctx, coreId)
		return { clearedCoreIds: [...coreIds] }
	},
})

async function ensureDevAccounts(ctx: MutationCtx, scenarios: number[]) {
	const accountIds: Record<string, string> = {}
	for (const scenario of scenarios) {
		const username = `scenario_${scenario}`
		const baseline = BASELINE_ACCOUNTS.find((account) => account.username === username)
		const displayName = baseline?.displayName ?? `Scenario ${scenario}`
		const friendCode = baseline?.friendCode ?? `AMB-${String(scenario).padStart(8, '0')}`
		const minecraftUuid = `00000000000040008000${String(scenario).padStart(12, '0')}`
		const existing = await ctx.db
			.query('users')
			.withIndex('by_normalized_username', (index) => index.eq('normalizedUsername', username))
			.unique()
		const now = Date.now()
		const value = {
			name: displayName,
			displayName,
			username,
			normalizedUsername: username,
			friendCode,
			amberiteUserId: `dev:${username}`,
			minecraftUuid,
			verifiedMinecraftHandle: username,
			normalizedVerifiedMinecraftHandle: username,
			minecraftLastVerifiedAt: now,
			onboardedAt: now,
		}
		if (existing) {
			await ctx.db.patch(existing._id, value)
			accountIds[username] = existing._id.toString()
		} else {
			const userId = await ctx.db.insert('users', { ...value, minecraftVerifiedAt: now })
			accountIds[username] = userId.toString()
		}
	}
	return accountIds
}

async function clearSocialState(ctx: MutationCtx) {
	for (const row of await ctx.db.query('socialNotifications').collect())
		await ctx.db.delete(row._id)
	for (const row of await ctx.db.query('friendRequests').collect()) await ctx.db.delete(row._id)
	for (const row of await ctx.db.query('friendships').collect()) await ctx.db.delete(row._id)
	for (const row of await ctx.db.query('blockedUsers').collect()) await ctx.db.delete(row._id)
	for (const row of await ctx.db.query('sharedClientUploads').collect())
		await ctx.db.delete(row._id)
	for (const row of await ctx.db.query('sharedClientVersions').collect())
		await ctx.db.delete(row._id)
	for (const row of await ctx.db.query('sharedClientInvites').collect())
		await ctx.db.delete(row._id)
	for (const row of await ctx.db.query('sharedClientMembers').collect())
		await ctx.db.delete(row._id)
	for (const row of await ctx.db.query('sharedClients').collect()) await ctx.db.delete(row._id)
	for (const row of await ctx.db.query('pairingCores').collect()) await ctx.db.delete(row._id)
	for (const row of await ctx.db.query('coreMemberLinks').collect()) await ctx.db.delete(row._id)
	for (const row of await ctx.db.query('coreList').collect()) await ctx.db.delete(row._id)
}

async function deleteCoreLinkState(ctx: MutationCtx, coreId: string) {
	const [core, links, pairing] = await Promise.all([
		ctx.db
			.query('coreList')
			.withIndex('by_core_id', (index) => index.eq('coreId', coreId))
			.unique(),
		ctx.db
			.query('coreMemberLinks')
			.withIndex('by_core', (index) => index.eq('coreId', coreId))
			.collect(),
		ctx.db
			.query('pairingCores')
			.withIndex('by_core_id', (index) => index.eq('coreId', coreId))
			.unique(),
	])
	for (const link of links) await ctx.db.delete(link._id)
	if (core) await ctx.db.delete(core._id)
	if (pairing) await ctx.db.delete(pairing._id)
}

function assertDev() {
	if (!isDevMode()) throw new Error('dev functions are disabled (AMBERITE_DEV_MODE not set)')
}

function assertScenario(scenario: number) {
	if (!Number.isSafeInteger(scenario) || scenario < 1 || scenario > 999_999_999_999)
		throw new Error('scenario must be a positive whole number')
}
