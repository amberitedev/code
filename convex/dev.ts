/**
 * Dev-only utilities for inspecting and resetting social state.
 *
 * Functions: devState, resetSocial, cleanupLegacyMockUsers.
 */
import { mutation, query } from './_generated/server'
import type { MutationCtx } from './_generated/server'
import { isDevMode } from './_socialRules'

function assertDev() {
	if (!isDevMode()) throw new Error('dev functions are disabled (AMBERITE_DEV_MODE not set)')
}

const LEGACY_MOCK_USERNAMES = [
	'dev-one', 'dev-two', 'amber', 'birch', 'cedar', 'dahlia', 'elm', 'fern', 'granite', 'hazel',
	'ivy', 'juniper', 'kepler', 'laurel', 'maple', 'nether', 'opal', 'pine',
] as const

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
	'userPresence',
	'blockedUsers',
	'friendGroupInvites',
	'friendGroupBans',
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
