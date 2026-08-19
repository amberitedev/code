import { v } from 'convex/values'
import { mutation } from './_generated/server'
import { canSendFriendRequest } from './_preferences'
import {
	acceptedFriendship,
	blockByPair,
	canonicalPair,
	friendRequestByPair,
	requireUserId,
} from './_socialRules'
import { dismissNotificationByDedupeKey, upsertSocialNotification } from './_socialNotifications'

export const addByCode = mutation({
	args: { code: v.string() },
	returns: v.null(),
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		const matches = await ctx.db
			.query('users')
			.withIndex('by_friend_code', (index) =>
				index.eq('friendCode', normalizeFriendCode(args.code)),
			)
			.take(2)
		const target = matches.find((user) => !user.deletedAt)
		if (matches.filter((user) => !user.deletedAt).length > 1)
			throw new Error('friend code is ambiguous')
		if (!target || target._id === actorId || target.deletedAt)
			throw new Error('friend code not found')
		const [blockedByActor, blockedByTarget] = await Promise.all([
			blockByPair(ctx, actorId, target._id),
			blockByPair(ctx, target._id, actorId),
		])
		if (blockedByActor || blockedByTarget) throw new Error('blocked users cannot interact')
		if (await acceptedFriendship(ctx, actorId, target._id))
			throw new Error('you are already friends with this user')

		const incoming = await friendRequestByPair(ctx, target._id, actorId)
		if (incoming?.status === 'pending') {
			const now = Date.now()
			const [userAId, userBId] = canonicalPair(actorId, target._id)
			await ctx.db.patch(incoming._id, { status: 'accepted', updatedAt: now })
			await ctx.db.insert('friendships', { userAId, userBId, createdAt: now })
			await dismissNotificationByDedupeKey(ctx, actorId, `friend-request:${incoming._id}`)
			await upsertSocialNotification(ctx, {
				userId: target._id,
				type: 'friend_request_accepted',
				actorUserId: actorId,
				friendRequestId: incoming._id,
				dedupeKey: `friend-accepted:${incoming._id}`,
			})
			return null
		}
		if (!(await canSendFriendRequest(ctx, actorId, target._id)))
			throw new Error('friend requests are disabled for this user')

		const existing = await friendRequestByPair(ctx, actorId, target._id)
		if (existing?.status === 'pending') throw new Error('friend request is already pending')
		const now = Date.now()
		const requestId = existing
			? (await ctx.db.patch(existing._id, { status: 'pending', updatedAt: now }), existing._id)
			: await ctx.db.insert('friendRequests', {
					fromUserId: actorId,
					toUserId: target._id,
					status: 'pending',
					createdAt: now,
					updatedAt: now,
				})
		await upsertSocialNotification(ctx, {
			userId: target._id,
			type: 'friend_request',
			actorUserId: actorId,
			friendRequestId: requestId,
			dedupeKey: `friend-request:${requestId}`,
		})
		return null
	},
})

function normalizeFriendCode(value: string): string {
	const code = value.trim().toUpperCase()
	if (!/^AMB-[A-Z0-9]{8}$/.test(code)) throw new Error('invalid friend code')
	return code
}
