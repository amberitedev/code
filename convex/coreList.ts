import { v } from 'convex/values'
import { query } from './_generated/server'
import type { Doc, Id } from './_generated/dataModel'
import type { QueryCtx } from './_generated/server'
import { requireUserId } from './_socialRules'

export const coreListEntryValidator = v.object({
	coreId: v.string(),
	ownerUserId: v.id('users'),
	linkState: v.union(v.literal('unlinked'), v.literal('linked')),
	connectionUrl: v.optional(v.string()),
	setupMode: v.optional(v.union(v.literal('remote'), v.literal('local'))),
	createdAt: v.number(),
	lastSeenAt: v.number(),
	projectionRevision: v.number(),
	syncedAt: v.number(),
	isOwner: v.boolean(),
	memberUserIds: v.array(v.id('users')),
})

export const listMine = query({
	args: {},
	returns: v.array(coreListEntryValidator),
	handler: async (ctx) => {
		const userId = await requireUserId(ctx)
		const rows = await ctx.db
			.query('coreList')
			.withIndex('by_owner', (q) => q.eq('ownerUserId', userId))
			.collect()
		return rows.map((core) => publicCoreListEntry(core, true))
	},
})

export const listLinkedForCurrent = query({
	args: {},
	returns: v.array(coreListEntryValidator),
	handler: async (ctx) => coreLinksForUser(ctx, await requireUserId(ctx)),
})

export async function coreLinksForUser(ctx: QueryCtx, userId: Id<'users'>) {
	const [memberLinks, ownedCores] = await Promise.all([
		ctx.db
			.query('coreMemberLinks')
			.withIndex('by_user', (q) => q.eq('userId', userId))
			.collect(),
		ctx.db
			.query('coreList')
			.withIndex('by_owner', (q) => q.eq('ownerUserId', userId))
			.collect(),
	])
	const byCoreId = new Map(ownedCores.map((core) => [core.coreId, core]))
	for (const link of memberLinks) {
		if (byCoreId.has(link.coreId)) continue
		const core = await ctx.db
			.query('coreList')
			.withIndex('by_core_id', (q) => q.eq('coreId', link.coreId))
			.unique()
		if (core) byCoreId.set(core.coreId, core)
	}
	const linkedByCoreId = new Map(memberLinks.map((link) => [link.coreId, link]))
	return Promise.all(
		[...byCoreId.values()]
			.filter((core) => core.linkState === 'linked' || core.ownerUserId === userId)
			.map(async (core) => {
				const coreMembers =
					core.linkState === 'linked'
						? await ctx.db
								.query('coreMemberLinks')
								.withIndex('by_core', (q) => q.eq('coreId', core.coreId))
								.collect()
						: []
				return publicCoreListEntry(
					core,
					linkedByCoreId.get(core.coreId)?.isOwner ?? core.ownerUserId === userId,
					coreMembers.map((member) => member.userId),
				)
			}),
	)
}

export async function coreMemberUserIdsForUser(
	ctx: QueryCtx,
	userId: Id<'users'>,
): Promise<Id<'users'>[]> {
	const links = await ctx.db
		.query('coreMemberLinks')
		.withIndex('by_user', (q) => q.eq('userId', userId))
		.collect()
	const members = new Set<Id<'users'>>()
	for (const link of links) {
		const core = await ctx.db
			.query('coreList')
			.withIndex('by_core_id', (q) => q.eq('coreId', link.coreId))
			.unique()
		if (!core || core.linkState !== 'linked') continue
		const coreLinks = await ctx.db
			.query('coreMemberLinks')
			.withIndex('by_core', (q) => q.eq('coreId', link.coreId))
			.collect()
		for (const coreLink of coreLinks) members.add(coreLink.userId)
	}
	return [...members]
}

function publicCoreListEntry(
	core: Doc<'coreList'>,
	isOwner: boolean,
	memberUserIds: Id<'users'>[] = [],
) {
	return {
		coreId: core.coreId,
		ownerUserId: core.ownerUserId,
		linkState: core.linkState,
		connectionUrl: core.connectionUrl,
		setupMode: core.setupMode,
		createdAt: core.createdAt,
		lastSeenAt: core.lastSeenAt,
		projectionRevision: core.projectionRevision,
		syncedAt: core.syncedAt,
		isOwner,
		memberUserIds,
	}
}
