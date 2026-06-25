import { v } from 'convex/values'
import { query } from './_generated/server'
import type { Id } from './_generated/dataModel'
import type { QueryCtx } from './_generated/server'
import { coreLinksForUser, coreMemberUserIdsForUser } from './coreList'
import {
	publicCurrentProfile,
	publicUser,
	resolveActor,
} from './_socialRules'

const devActAs = { __actAs: v.optional(v.string()) }

/** The single durable social subscription for the desktop shell. */
export const sessionState = query({
	args: { ...devActAs },
	handler: async (ctx, args) => sessionStateForUser(ctx, await resolveActor(ctx, args.__actAs)),
})

export async function sessionStateForUser(ctx: QueryCtx, userId: Id<'users'>) {
	const user = await ctx.db.get(userId)
	if (!user) throw new Error('user not found')
	const [left, right, incoming, outgoing, blocks, coreLinks] = await Promise.all([
		ctx.db
			.query('friendships')
			.withIndex('by_user_a', (q) => q.eq('userAId', userId))
			.collect(),
		ctx.db
			.query('friendships')
			.withIndex('by_user_b', (q) => q.eq('userBId', userId))
			.collect(),
		ctx.db
			.query('friendRequests')
			.withIndex('by_to_status', (q) => q.eq('toUserId', userId).eq('status', 'pending'))
			.collect(),
		ctx.db
			.query('friendRequests')
			.withIndex('by_from_status', (q) => q.eq('fromUserId', userId).eq('status', 'pending'))
			.collect(),
		ctx.db
			.query('blockedUsers')
			.withIndex('by_blocker', (q) => q.eq('blockerUserId', userId))
			.collect(),
		coreLinksForUser(ctx, userId),
	])
	const friendships = await Promise.all(
		[...left, ...right].map(async (friendship) => {
			const otherId = friendship.userAId === userId ? friendship.userBId : friendship.userAId
			const other = await ctx.db.get(otherId as Id<'users'>)
			return other
				? { friendshipId: friendship._id, user: publicUser(other), createdAt: friendship.createdAt }
				: null
		}),
	)
	const requests = async (items: typeof incoming, field: 'fromUserId' | 'toUserId') =>
		Promise.all(
			items.map(async (request) => {
				const requestUser = await ctx.db.get(request[field] as Id<'users'>)
				return {
					request: publicFriendRequest(request),
					user: requestUser ? publicUser(requestUser) : null,
				}
			}),
		)
	return {
		currentUser: publicUser(user, true),
		profile: publicCurrentProfile(user),
		friends: {
			friends: friendships.filter(
				(friendship): friendship is NonNullable<typeof friendship> => friendship !== null,
			),
			incoming: await requests(incoming, 'fromUserId'),
			outgoing: await requests(outgoing, 'toUserId'),
			blocks: await Promise.all(
				blocks.map(async (block) => {
					const blockedUser = await ctx.db.get(block.blockedUserId as Id<'users'>)
					return {
						blockId: block._id,
						user: blockedUser ? publicUser(blockedUser) : null,
						createdAt: block.createdAt,
					}
				}),
			),
		},
		coreLinks,
	}
}

function publicFriendRequest(request: any) {
	return {
		_id: request._id,
		fromUserId: request.fromUserId,
		toUserId: request.toUserId,
		status: request.status,
		message: request.message,
		createdAt: request.createdAt,
		updatedAt: request.updatedAt,
	}
}

export async function liveScopeForUser(ctx: QueryCtx, userId: Id<'users'>) {
	const [left, right, memberUserIds] = await Promise.all([
		ctx.db
			.query('friendships')
			.withIndex('by_user_a', (q) => q.eq('userAId', userId))
			.collect(),
		ctx.db
			.query('friendships')
			.withIndex('by_user_b', (q) => q.eq('userBId', userId))
			.collect(),
		coreMemberUserIdsForUser(ctx, userId),
	])
	const friendUserIds = [...left, ...right].map((friendship) =>
		friendship.userAId === userId ? friendship.userBId : friendship.userAId,
	)
	return { userId, friendUserIds, memberUserIds }
}
