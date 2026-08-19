/**
 * Dev-only utilities for account baselines, inspection, and state resets.
 *
 * Functions: ensureAccounts, applyBaseline, devState, clearCoreLink, resetSocial,
 * cleanupLegacyMockUsers, repairLegacyMinecraftIdentity.
 */
import { v } from 'convex/values'
import { internalMutation, mutation, query } from './_generated/server'
import type { Id } from './_generated/dataModel'
import type { MutationCtx } from './_generated/server'
import { isDevMode, publicLegacyCore, requireUserId } from './_socialRules'
import { normalizeMinecraftUuid } from './minecraftIdentity'

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
	{
		username: 'scenario_1',
		displayName: 'Owner',
		friendCode: 'AMB-OWNER001',
		minecraftUuid: '00000000-0000-4000-8000-000000000001',
	},
	{
		username: 'scenario_2',
		displayName: 'Admin',
		friendCode: 'AMB-ADMIN001',
		minecraftUuid: '00000000-0000-4000-8000-000000000002',
	},
	{
		username: 'scenario_3',
		displayName: 'Member',
		friendCode: 'AMB-MEMBER01',
		minecraftUuid: '00000000-0000-4000-8000-000000000003',
	},
	{
		username: 'scenario_4',
		displayName: 'Invited',
		friendCode: 'AMB-INVITE01',
		minecraftUuid: '00000000-0000-4000-8000-000000000004',
	},
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

/** Ensure every App scenario has a deterministic fake Amberite account. */
export const ensureScenarios = mutation({
	args: { scenarios: v.array(v.number()) },
	handler: async (ctx, args) => {
		assertDev()
		const scenarios = [...new Set(args.scenarios)]
		for (const scenario of scenarios) assertScenario(scenario)
		return await ensureDevAccounts(ctx, scenarios.map(accountForScenario))
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
			ownerUserId: accounts.scenario_1,
			createdAt: now,
			updatedAt: now,
		})
		await Promise.all([
			ctx.db.insert('friendGroupMembers', {
				friendGroupId: friendGroupId.toString(),
				userId: accounts.scenario_1,
				role: 'owner',
				permissionPreset: 'owner',
				createdAt: now,
				updatedAt: now,
			}),
			ctx.db.insert('friendGroupMembers', {
				friendGroupId: friendGroupId.toString(),
				userId: accounts.scenario_2,
				role: 'admin',
				permissionPreset: 'admin',
				createdAt: now,
				updatedAt: now,
			}),
			ctx.db.insert('friendGroupMembers', {
				friendGroupId: friendGroupId.toString(),
				userId: accounts.scenario_3,
				role: 'member',
				permissionPreset: 'member',
				createdAt: now,
				updatedAt: now,
			}),
			ctx.db.insert('friendGroupInvites', {
				friendGroupId: friendGroupId.toString(),
				inviterUserId: accounts.scenario_1,
				inviteeUserId: accounts.scenario_4,
				role: 'member',
				status: 'pending',
				createdAt: now,
				expiresAt: now + 7 * 24 * 60 * 60 * 1000,
			}),
		])
		return { baseline: args.baseline, accounts, friendGroupId }
	},
})

async function ensureBaselineAccounts(ctx: MutationCtx) {
	return await ensureDevAccounts(ctx, BASELINE_ACCOUNTS)
}

type DevAccount = {
	readonly displayName: string
	readonly friendCode: string
	readonly minecraftUuid: string
	readonly username: string
}

async function ensureDevAccounts(ctx: MutationCtx, accounts: ReadonlyArray<DevAccount>) {
	const accountIds: Record<string, string> = {}
	for (const account of accounts) {
		const existing = await ctx.db
			.query('users')
			.withIndex('by_normalized_username', (q) => q.eq('normalizedUsername', account.username))
			.unique()
		if (existing) {
			await ctx.db.patch(existing._id, {
				minecraftUuid: account.minecraftUuid,
				verifiedMinecraftHandle: account.username,
				normalizedVerifiedMinecraftHandle: account.username,
				minecraftVerifiedAt: existing.minecraftVerifiedAt ?? Date.now(),
				minecraftLastVerifiedAt: Date.now(),
			})
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
			minecraftUuid: account.minecraftUuid,
			verifiedMinecraftHandle: account.username,
			normalizedVerifiedMinecraftHandle: account.username,
			minecraftVerifiedAt: Date.now(),
			minecraftLastVerifiedAt: Date.now(),
			onboardedAt: Date.now(),
		}
		const userId = await ctx.db.insert('users', values)
		accountIds[account.username] = userId.toString()
	}
	return accountIds
}

function accountForScenario(scenario: number): DevAccount {
	const baseline = BASELINE_ACCOUNTS.find((account) => account.username === `scenario_${scenario}`)
	if (baseline) return baseline
	const suffix = String(scenario).padStart(12, '0')
	return {
		username: `scenario_${scenario}`,
		displayName: `Scenario ${scenario}`,
		friendCode: `AMB-DEV-${scenario}`,
		minecraftUuid: `00000000-0000-4000-8000-${suffix}`,
	}
}

function assertScenario(scenario: number): void {
	if (!Number.isSafeInteger(scenario) || scenario < 1 || scenario > 999_999_999_999) {
		throw new Error('scenario must be a positive whole number')
	}
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

/** Retire duplicate legacy users only when the canonical Minecraft account is provable. */
export const repairLegacyMinecraftIdentity = internalMutation({
	args: { minecraftUuid: v.string() },
	returns: v.object({
		canonicalUserId: v.id('users'),
		retiredUsers: v.number(),
		removedSessions: v.number(),
		removedSelfRelationships: v.number(),
	}),
	handler: async (ctx, args) => {
		assertDev()
		const minecraftUuid = normalizeMinecraftUuid(args.minecraftUuid)
		const canonicalAccountId = `minecraft:${minecraftUuid}`
		const canonicalAccounts = await ctx.db
			.query('authAccounts')
			.withIndex('providerAndAccountId', (q) =>
				q.eq('provider', 'minecraft-token').eq('providerAccountId', canonicalAccountId),
			)
			.take(2)
		if (canonicalAccounts.length !== 1)
			throw new Error('A unique canonical Minecraft provider account is required')
		const canonicalUser = await ctx.db.get(canonicalAccounts[0].userId)
		if (!canonicalUser || canonicalUser.deletedAt)
			throw new Error('The canonical Minecraft provider account has no live user')

		const [directUsers, links] = await Promise.all([
			ctx.db
				.query('users')
				.withIndex('by_minecraft_uuid', (q) => q.eq('minecraftUuid', minecraftUuid))
				.collect(),
			ctx.db
				.query('linkedMicrosoftAccounts')
				.withIndex('by_minecraft_uuid', (q) => q.eq('minecraftUuid', minecraftUuid))
				.collect(),
		])
		const claimants = new Map<string, Id<'users'>>()
		for (const user of directUsers) {
			if (!user.deletedAt) claimants.set(user._id.toString(), user._id)
		}
		for (const link of links) {
			const users = await ctx.db
				.query('users')
				.withIndex('by_amberite_user_id', (q) => q.eq('amberiteUserId', link.amberiteUserId))
				.take(2)
			if (users.length > 1) throw new Error('A legacy Amberite user ID maps to multiple users')
			if (users[0] && !users[0].deletedAt) claimants.set(users[0]._id.toString(), users[0]._id)
		}
		if (!claimants.has(canonicalUser._id.toString()))
			throw new Error('The canonical user does not claim this Minecraft identity')
		const duplicateIds = new Set(
			[...claimants.keys()].filter((userId) => userId !== canonicalUser._id.toString()),
		)
		if (duplicateIds.size === 0)
			return {
				canonicalUserId: canonicalUser._id,
				retiredUsers: 0,
				removedSessions: 0,
				removedSelfRelationships: 0,
			}

		const [
			members,
			requests,
			friendships,
			blocks,
			invites,
			bans,
			groups,
			pairingCores,
			coreList,
			coreMemberLinks,
			cores,
			syncedProfiles,
			profileSnapshots,
			modSyncEvents,
			messages,
			receipts,
		] = await Promise.all([
			ctx.db.query('friendGroupMembers').collect(),
			ctx.db.query('friendRequests').collect(),
			ctx.db.query('friendships').collect(),
			ctx.db.query('blockedUsers').collect(),
			ctx.db.query('friendGroupInvites').collect(),
			ctx.db.query('friendGroupBans').collect(),
			ctx.db.query('friendGroups').collect(),
			ctx.db.query('pairingCores').collect(),
			ctx.db.query('coreList').collect(),
			ctx.db.query('coreMemberLinks').collect(),
			ctx.db.query('cores').collect(),
			ctx.db.query('syncedProfiles').collect(),
			ctx.db.query('profileSnapshots').collect(),
			ctx.db.query('modSyncEvents').collect(),
			ctx.db.query('messages').collect(),
			ctx.db.query('receipts').collect(),
		])
		const claimantIds = new Set(claimants.keys())
		const selfRelationshipIds = new Set<Id<'friendRequests' | 'friendships' | 'blockedUsers'>>()
		for (const request of requests) {
			if (!duplicateIds.has(request.fromUserId) && !duplicateIds.has(request.toUserId)) continue
			if (!claimantIds.has(request.fromUserId) || !claimantIds.has(request.toUserId))
				throw new Error('A duplicate user has an external friend request')
			selfRelationshipIds.add(request._id)
		}
		for (const friendship of friendships) {
			if (!duplicateIds.has(friendship.userAId) && !duplicateIds.has(friendship.userBId)) continue
			if (!claimantIds.has(friendship.userAId) || !claimantIds.has(friendship.userBId))
				throw new Error('A duplicate user has an external friendship')
			selfRelationshipIds.add(friendship._id)
		}
		for (const block of blocks) {
			if (!duplicateIds.has(block.blockerUserId) && !duplicateIds.has(block.blockedUserId)) continue
			if (!claimantIds.has(block.blockerUserId) || !claimantIds.has(block.blockedUserId))
				throw new Error('A duplicate user has an external block')
			selfRelationshipIds.add(block._id)
		}

		const duplicateMembershipIds = new Set<Id<'friendGroupMembers'>>()
		for (const member of members) {
			if (!duplicateIds.has(member.userId)) continue
			const canonicalMembership = members.find(
				(candidate) =>
					candidate.friendGroupId === member.friendGroupId &&
					candidate.userId === canonicalUser._id.toString(),
			)
			if (!canonicalMembership || roleRank(canonicalMembership.role) < roleRank(member.role))
				throw new Error('A duplicate user has group membership that cannot be merged safely')
			duplicateMembershipIds.add(member._id)
		}
		const duplicateInviteIds = new Set<Id<'friendGroupInvites'>>()
		for (const invite of invites) {
			const touchesDuplicate =
				duplicateIds.has(invite.inviterUserId) ||
				(invite.inviteeUserId !== undefined && duplicateIds.has(invite.inviteeUserId))
			if (!touchesDuplicate) continue
			if (
				!claimantIds.has(invite.inviterUserId) ||
				!invite.inviteeUserId ||
				!claimantIds.has(invite.inviteeUserId)
			)
				throw new Error('A duplicate user has an external group invite')
			duplicateInviteIds.add(invite._id)
		}

		const hasUnsupportedReference =
			bans.some((ban) => duplicateIds.has(ban.userId) || duplicateIds.has(ban.bannedByUserId)) ||
			groups.some((group) => duplicateIds.has(group.ownerUserId)) ||
			pairingCores.some(
				(pairing) => pairing.ownerUserId !== undefined && duplicateIds.has(pairing.ownerUserId),
			) ||
			coreList.some((core) => duplicateIds.has(core.ownerUserId)) ||
			coreMemberLinks.some((link) => duplicateIds.has(link.userId)) ||
			cores.some((core) => duplicateIds.has(core.ownerUserId)) ||
			syncedProfiles.some(
				(profile) =>
					profile.visibilityUserIds?.some((userId) => duplicateIds.has(userId)) ||
					profile.whitelistUserIds?.some((userId) => duplicateIds.has(userId)),
			) ||
			profileSnapshots.some((snapshot) => duplicateIds.has(snapshot.authorUserId)) ||
			modSyncEvents.some((event) => duplicateIds.has(event.authorUserId)) ||
			messages.some(
				(message) => duplicateIds.has(message.senderId) || duplicateIds.has(message.recipientId),
			) ||
			receipts.some((receipt) => duplicateIds.has(receipt.recipientId))
		if (hasUnsupportedReference)
			throw new Error('A duplicate user has product data that requires a manual merge')

		for (const rowId of selfRelationshipIds) await ctx.db.delete(rowId)
		for (const rowId of duplicateMembershipIds) await ctx.db.delete(rowId)
		for (const rowId of duplicateInviteIds) await ctx.db.delete(rowId)

		let removedSessions = 0
		for (const duplicateId of duplicateIds) {
			const duplicateUser = await ctx.db.get(duplicateId as Id<'users'>)
			if (!duplicateUser || duplicateUser.deletedAt) continue
			const [authAccounts, modrinthAccounts, sessions] = await Promise.all([
				ctx.db
					.query('authAccounts')
					.withIndex('userIdAndProvider', (q) => q.eq('userId', duplicateUser._id))
					.collect(),
				ctx.db
					.query('linkedModrinthAccounts')
					.withIndex('by_user', (q) => q.eq('userId', duplicateUser._id))
					.collect(),
				ctx.db
					.query('authSessions')
					.withIndex('userId', (q) => q.eq('userId', duplicateUser._id))
					.collect(),
			])
			if (modrinthAccounts.length > 0)
				throw new Error('A duplicate user has a linked Modrinth account')
			if (
				!duplicateUser.amberiteUserId ||
				authAccounts.some(
					(account) =>
						account.provider !== 'minecraft-token' ||
						account.providerAccountId !== duplicateUser.amberiteUserId,
				)
			)
				throw new Error('A duplicate user has a non-legacy auth account')
			for (const session of sessions) {
				const refreshTokens = await ctx.db
					.query('authRefreshTokens')
					.withIndex('sessionId', (q) => q.eq('sessionId', session._id))
					.collect()
				for (const refreshToken of refreshTokens) await ctx.db.delete(refreshToken._id)
				await ctx.db.delete(session._id)
				removedSessions += 1
			}
			for (const account of authAccounts) await ctx.db.delete(account._id)
			const duplicateLinks = await ctx.db
				.query('linkedMicrosoftAccounts')
				.withIndex('by_amberite_user', (q) => q.eq('amberiteUserId', duplicateUser.amberiteUserId!))
				.collect()
			for (const link of duplicateLinks) await ctx.db.delete(link._id)
			await ctx.db.patch(duplicateUser._id, {
				deletedAt: Date.now(),
				deletedReason: `retired duplicate Minecraft identity; canonical user ${canonicalUser._id}`,
				amberiteUserId: undefined,
				friendCode: undefined,
				minecraftUuid: undefined,
				verifiedMinecraftHandle: undefined,
				normalizedVerifiedMinecraftHandle: undefined,
			})
		}

		return {
			canonicalUserId: canonicalUser._id,
			retiredUsers: duplicateIds.size,
			removedSessions,
			removedSelfRelationships:
				selfRelationshipIds.size + duplicateMembershipIds.size + duplicateInviteIds.size,
		}
	},
})

function roleRank(role: 'owner' | 'admin' | 'member'): number {
	return role === 'owner' ? 3 : role === 'admin' ? 2 : 1
}

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
