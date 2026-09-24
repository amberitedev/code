import { v } from 'convex/values'
import { mutation } from './_generated/server'
import { isDevMode, requireUserId } from './_socialRules'

export const linkLocal = mutation({
	args: { coreId: v.string(), connectionUrl: v.string() },
	returns: v.object({ coreId: v.string() }),
	handler: async (ctx, args) => {
		if (!isDevMode()) throw new Error('local Core linking is only available in development')
		const userId = await requireUserId(ctx)
		const connectionUrl = localCoreUrl(args.connectionUrl)
		const coreId = args.coreId.trim()
		if (!coreId || coreId.length > 256) throw new Error('invalid Core id')

		const now = Date.now()
		const existingCore = await ctx.db
			.query('coreList')
			.withIndex('by_core_id', (index) => index.eq('coreId', coreId))
			.unique()
		if (existingCore && existingCore.ownerUserId !== userId)
			throw new Error('Core already belongs to another user')

		const coreValue = {
			ownerUserId: userId,
			linkState: 'linked' as const,
			connectionUrl,
			setupMode: 'local' as const,
			lastSeenAt: now,
			projectionRevision: existingCore?.projectionRevision ?? 0,
			syncedAt: now,
		}
		if (existingCore) await ctx.db.patch(existingCore._id, coreValue)
		else await ctx.db.insert('coreList', { ...coreValue, coreId, createdAt: now })

		const ownerLink = await ctx.db
			.query('coreMemberLinks')
			.withIndex('by_core_user', (index) => index.eq('coreId', coreId).eq('userId', userId))
			.unique()
		const linkValue = { coreId, userId, isOwner: true, syncedAt: now }
		if (ownerLink) await ctx.db.patch(ownerLink._id, linkValue)
		else await ctx.db.insert('coreMemberLinks', linkValue)

		return { coreId }
	},
})

function localCoreUrl(value: string) {
	const url = new URL(value)
	if (
		url.protocol !== 'http:' ||
		!['127.0.0.1', 'localhost', '::1'].includes(url.hostname) ||
		url.username ||
		url.password ||
		url.pathname !== '/' ||
		url.search ||
		url.hash
	)
		throw new Error('development Core URL must be a local HTTP origin')
	return url.origin
}
