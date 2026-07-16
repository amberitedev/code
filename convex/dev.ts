/**
 * Dev-only utilities for account baselines, inspection, and state resets.
 *
 * Functions: ensureAccounts, applyBaseline, devState, clearCoreLink, resetSocial,
 * cleanupLegacyMockUsers.
 */
import { v } from 'convex/values'
import { mutation, query } from './_generated/server'
import type { Id } from './_generated/dataModel'
import type { MutationCtx } from './_generated/server'
import { isDevMode, publicLegacyCore, requireUserId } from './_socialRules'

function assertDev() {
	if (!isDevMode()) throw new Error('dev functions are disabled (AMBERITE_DEV_MODE not set)')
}

const LEGACY_MOCK_USERNAMES = [
	'dev-one',
	'dev-two',
	'amber',
	'birch',
	'cedar',
	'dahlia',
	'elm',
	'fern',
	'granite',
	'hazel',
	'ivy',
	'juniper',
	'kepler',
	'laurel',
	'maple',
	'nether',
	'opal',
	'pine',
] as const

const BASELINE_ACCOUNTS = [
	{ username: 'owner', displayName: 'Owner', friendCode: 'AMB-OWNER001' },
	{ username: 'friend', displayName: 'Friend', friendCode: 'AMB-FRIEND01' },
	{ username: 'other', displayName: 'Other', friendCode: 'AMB-OTHER001' },
] as const

const BASELINE_TABLES = [
	'authRefreshTokens',
	'authVerificationCodes',
	'authVerifiers',
	'authSessions',
	'authAccounts',
	'authRateLimits',
	'linkedMicrosoftAccounts',
	'linkedModrinthAccounts',
	'friendGroups',
	'friendGroupMembers',
	'friendRequests',
	'friendships',
	'blockedUsers',
	'friendGroupInvites',
	'friendGroupBans',
	'coreList',
	'coreMemberLinks',
	'cores',
	'pairingCores',
	'syncedProfiles',
	'profileSnapshots',
	'modSyncEvents',
	'messages',
	'receipts',
	'realtimeBridgeRequests',
	'users',
] as const

/** Ensure the shared cloud development deployment has the standard login accounts. */
export const ensureAccounts = mutation({
	args: {},
	handler: async (ctx) => {
		assertDev()
		return await ensureBaselineAccounts(ctx)
	},
})

/** Replace a local deployment with one of Amberite's two deterministic baselines. */
export const applyBaseline = mutation({
	args: { baseline: v.union(v.literal('accounts'), v.literal('group')) },
	handler: async (ctx, args) => {
		assertDev()
		for (const table of BASELINE_TABLES) {
			const rows = await ctx.db.query(table).collect()
			for (const row of rows) await ctx.db.delete(row._id)
		}

		const accounts = await ensureBaselineAccounts(ctx)
		if (args.baseline === 'accounts') return { baseline: args.baseline, accounts }

		const now = Date.now()
		const friendGroupId = await ctx.db.insert('friendGroups', {
			name: 'Development Group',
			description: 'Disconnected group baseline',
			ownerUserId: accounts.owner,
			createdAt: now,
			updatedAt: now,
		})
		await Promise.all([
			ctx.db.insert('friendGroupMembers', {
				friendGroupId: friendGroupId.toString(),
				userId: accounts.owner,
				role: 'owner',
				createdAt: now,
				updatedAt: now,
			}),
			ctx.db.insert('friendGroupMembers', {
				friendGroupId: friendGroupId.toString(),
				userId: accounts.friend,
				role: 'member',
				createdAt: now,
				updatedAt: now,
			}),
		])
		return { baseline: args.baseline, accounts, friendGroupId }
	},
})

async function ensureBaselineAccounts(ctx: MutationCtx) {
	const accountIds: Record<(typeof BASELINE_ACCOUNTS)[number]['username'], string> = {
		owner: '',
		friend: '',
		other: '',
	}
	for (const account of BASELINE_ACCOUNTS) {
		const existing = await ctx.db
			.query('users')
			.withIndex('by_normalized_username', (q) =>
				q.eq('normalizedUsername', account.username),
			)
			.unique()
		if (existing) {
			accountIds[account.username] = existing._id.toString()
			continue
		}
		const values = {
			name: account.displayName,
			displayName: account.displayName,
			username: account.username,
			normalizedUsername: account.username,
			friendCode: account.friendCode,
			amberiteUserId: `dev:${account.username}`,
			onboardedAt: Date.now(),
		}
		const userId = await ctx.db.insert('users', values)
		accountIds[account.username] = userId.toString()
	}
	return accountIds
}

/** Full snapshot of social state — handy for inspecting flows from DevTools. */
export const devState = query({
	args: {},
	handler: async (ctx) => {
		assertDev()
		const [
			groups,
			members,
			requests,
			friendships,
			invites,
			bans,
			coreList,
			coreMemberLinks,
			cores,
			pairing,
		] = await Promise.all([
			ctx.db.query('friendGroups').collect(),
			ctx.db.query('friendGroupMembers').collect(),
			ctx.db.query('friendRequests').collect(),
			ctx.db.query('friendships').collect(),
			ctx.db.query('friendGroupInvites').collect(),
			ctx.db.query('friendGroupBans').collect(),
			ctx.db.query('coreList').collect(),
			ctx.db.query('coreMemberLinks').collect(),
			ctx.db.query('cores').collect(),
			ctx.db.query('pairingCores').collect(),
		])
		return {
			groups,
			members,
			requests,
			friendships,
			invites,
			bans,
			coreList,
			coreMemberLinks,
			cores: cores.map(publicLegacyCore),
			pairing: pairing.map((core) => ({
				...core,
				realtimeCredentialHash: undefined,
			})),
		}
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
	'coreList',
	'coreMemberLinks',
	'cores',
	'pairingCores',
	'syncedProfiles',
	'profileSnapshots',
	'modSyncEvents',
] as const

async function clearSocialState(ctx: MutationCtx) {
	for (const table of SOCIAL_TABLES) {
		const rows = await ctx.db.query(table).collect()
		for (const row of rows) await ctx.db.delete(row._id)
	}
}

/** Wipe social state while keeping real user accounts and auth sessions. */
export const resetSocial = mutation({
	args: {},
	handler: async (ctx) => {
		assertDev()
		await clearSocialState(ctx)
		return null
	},
})

/** Clear the current user's linked Core records so the desktop app can test pairing again. */
export const clearCoreLink = mutation({
	args: { coreId: v.optional(v.string()) },
	handler: async (ctx, args) => {
		assertDev()
		const userId = await requireUserId(ctx)
		const coreIds = args.coreId
			? await ownedCoreIdsForRequest(ctx, userId, args.coreId)
			: await ownedCoreIdsForUser(ctx, userId)

		for (const coreId of coreIds) await deleteCoreLinkState(ctx, coreId)
		return { clearedCoreIds: [...coreIds] }
	},
})

/** Remove retired mock identities and all social data without touching real users or auth sessions. */
export const cleanupLegacyMockUsers = mutation({
	args: {},
	handler: async (ctx) => {
		assertDev()
		await clearSocialState(ctx)
		let removedUsers = 0
		for (const username of LEGACY_MOCK_USERNAMES) {
			const user = await ctx.db
				.query('users')
				.withIndex('by_normalized_username', (q) => q.eq('normalizedUsername', username))
				.unique()
			if (!user) continue
			if (user.amberiteUserId) {
				const accounts = await ctx.db
					.query('linkedMicrosoftAccounts')
					.withIndex('by_amberite_user', (q) => q.eq('amberiteUserId', user.amberiteUserId!))
					.collect()
				for (const account of accounts) await ctx.db.delete(account._id)
			}
			await ctx.db.delete(user._id)
			removedUsers += 1
		}
		return { removedUsers }
	},
})

async function ownedCoreIdsForRequest(ctx: MutationCtx, userId: string, coreId: string) {
	const [coreList, memberLink, legacyCore] = await Promise.all([
		ctx.db
			.query('coreList')
			.withIndex('by_core_id', (q) => q.eq('coreId', coreId))
			.unique(),
		ctx.db
			.query('coreMemberLinks')
			.withIndex('by_core_user', (q) => q.eq('coreId', coreId).eq('userId', userId))
			.unique(),
		ctx.db
			.query('cores')
			.withIndex('by_core_id', (q) => q.eq('coreId', coreId))
			.unique(),
	])
	if (!coreList && !memberLink && !legacyCore) return new Set([coreId])
	if (
		(!coreList || coreList.ownerUserId !== userId) &&
		memberLink?.isOwner !== true &&
		(!legacyCore || legacyCore.ownerUserId !== userId)
	) {
		throw new Error('Core link not found for current user')
	}
	return new Set([coreId])
}

async function ownedCoreIdsForUser(ctx: MutationCtx, userId: string) {
	const [ownedCoreList, memberLinks, legacyCores] = await Promise.all([
		ctx.db
			.query('coreList')
			.withIndex('by_owner', (q) => q.eq('ownerUserId', userId))
			.collect(),
		ctx.db
			.query('coreMemberLinks')
			.withIndex('by_user', (q) => q.eq('userId', userId))
			.collect(),
		ctx.db
			.query('cores')
			.withIndex('by_owner', (q) => q.eq('ownerUserId', userId))
			.collect(),
	])
	const coreIds = new Set<string>()
	for (const core of ownedCoreList) coreIds.add(core.coreId)
	for (const link of memberLinks) {
		if (link.isOwner) coreIds.add(link.coreId)
	}
	for (const core of legacyCores) coreIds.add(core.coreId)
	return coreIds
}

async function deleteCoreLinkState(ctx: MutationCtx, coreId: string) {
	const [coreList, memberLinks, legacyCore, pairingCore, friendGroups, syncedProfiles] =
		await Promise.all([
			ctx.db
				.query('coreList')
				.withIndex('by_core_id', (q) => q.eq('coreId', coreId))
				.unique(),
			ctx.db
				.query('coreMemberLinks')
				.withIndex('by_core', (q) => q.eq('coreId', coreId))
				.collect(),
			ctx.db
				.query('cores')
				.withIndex('by_core_id', (q) => q.eq('coreId', coreId))
				.unique(),
			ctx.db
				.query('pairingCores')
				.withIndex('by_core_id', (q) => q.eq('coreId', coreId))
				.unique(),
			ctx.db
				.query('friendGroups')
				.withIndex('by_core_id', (q) => q.eq('coreId', coreId))
				.collect(),
			ctx.db.query('syncedProfiles').collect(),
		])

	for (const profile of syncedProfiles) {
		if (profile.coreId !== coreId) continue
		await deleteSyncedProfileState(ctx, profile._id.toString())
		await ctx.db.delete(profile._id)
	}
	for (const group of friendGroups) await deleteFriendGroupState(ctx, group._id.toString())
	for (const link of memberLinks) await ctx.db.delete(link._id)
	if (coreList) await ctx.db.delete(coreList._id)
	if (legacyCore) await ctx.db.delete(legacyCore._id)
	if (pairingCore) await ctx.db.delete(pairingCore._id)
}

async function deleteFriendGroupState(ctx: MutationCtx, friendGroupId: string) {
	const [members, bans, invites, profiles] = await Promise.all([
		ctx.db
			.query('friendGroupMembers')
			.withIndex('by_group', (q) => q.eq('friendGroupId', friendGroupId))
			.collect(),
		ctx.db
			.query('friendGroupBans')
			.withIndex('by_group', (q) => q.eq('friendGroupId', friendGroupId))
			.collect(),
		ctx.db.query('friendGroupInvites').collect(),
		ctx.db
			.query('syncedProfiles')
			.withIndex('by_group', (q) => q.eq('friendGroupId', friendGroupId))
			.collect(),
	])
	for (const profile of profiles) {
		await deleteSyncedProfileState(ctx, profile._id.toString())
		await ctx.db.delete(profile._id)
	}
	for (const invite of invites) {
		if (invite.friendGroupId === friendGroupId) await ctx.db.delete(invite._id)
	}
	for (const ban of bans) await ctx.db.delete(ban._id)
	for (const member of members) await ctx.db.delete(member._id)
	const group = await ctx.db.get(friendGroupId as Id<'friendGroups'>)
	if (group) await ctx.db.delete(group._id)
}

async function deleteSyncedProfileState(ctx: MutationCtx, profileId: string) {
	const [snapshots, events] = await Promise.all([
		ctx.db
			.query('profileSnapshots')
			.withIndex('by_profile', (q) => q.eq('profileId', profileId))
			.collect(),
		ctx.db
			.query('modSyncEvents')
			.withIndex('by_profile', (q) => q.eq('profileId', profileId))
			.collect(),
	])
	for (const event of events) await ctx.db.delete(event._id)
	for (const snapshot of snapshots) await ctx.db.delete(snapshot._id)
}
