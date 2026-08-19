import { v } from 'convex/values'
import type { Id } from './_generated/dataModel'
import { query } from './_generated/server'
import type { QueryCtx } from './_generated/server'
import { coreLinksForUser, coreListEntryValidator, coreMemberUserIdsForUser } from './coreList'
import {
	currentAccountFields,
	publicCurrentProfileValidator,
	publicCurrentProfile,
	publicUser,
	publicUserValidator,
	resolveActor,
} from './_socialRules'

const devActAs = { __actAs: v.optional(v.string()) }
const friendRequestValidator = v.object({
	_id: v.id('friendRequests'),
	fromUserId: v.id('users'),
	toUserId: v.id('users'),
	status: v.union(
		v.literal('pending'),
		v.literal('accepted'),
		v.literal('declined'),
		v.literal('canceled'),
	),
	createdAt: v.number(),
	updatedAt: v.number(),
})
const friendRequestWithUserValidator = v.object({
	request: friendRequestValidator,
	user: v.union(publicUserValidator, v.null()),
})

/** One durable subscription for account, friends, and explicitly paired Core links. */
export const sessionState = query({
	args: { ...devActAs },
	returns: v.object({
		currentUser: publicUserValidator,
		profile: publicCurrentProfileValidator,
		friends: v.object({
			friends: v.array(
				v.object({
					friendshipId: v.id('friendships'),
					user: publicUserValidator,
					createdAt: v.number(),
				}),
			),
			incoming: v.array(friendRequestWithUserValidator),
			outgoing: v.array(friendRequestWithUserValidator),
			blocks: v.array(
				v.object({
					blockId: v.id('blockedUsers'),
					user: v.union(publicUserValidator, v.null()),
					createdAt: v.number(),
				}),
			),
		}),
		coreLinks: v.array(coreListEntryValidator),
		group: v.null(),
		members: v.array(v.null()),
		bans: v.array(v.null()),
		pendingInvites: v.array(v.null()),
		core: v.null(),
	}),
	handler: async (ctx, args) => sessionStateForUser(ctx, await resolveActor(ctx, args.__actAs)),
})

export async function sessionStateForUser(ctx: QueryCtx, userId: Id<'users'>) {
	const user = await ctx.db.get(userId)
	if (!user || user.deletedAt) throw new Error('user not found')
	const accountFields = await currentAccountFields(ctx, userId)
	const [left, right, incoming, outgoing, blocks, coreLinks] = await Promise.all([
		ctx.db
			.query('friendships')
			.withIndex('by_user_a', (index) => index.eq('userAId', userId))
			.take(100),
		ctx.db
			.query('friendships')
			.withIndex('by_user_b', (index) => index.eq('userBId', userId))
			.take(100),
		ctx.db
			.query('friendRequests')
			.withIndex('by_to_status', (index) => index.eq('toUserId', userId).eq('status', 'pending'))
			.take(100),
		ctx.db
			.query('friendRequests')
			.withIndex('by_from_status', (index) =>
				index.eq('fromUserId', userId).eq('status', 'pending'),
			)
			.take(100),
		ctx.db
			.query('blockedUsers')
			.withIndex('by_blocker', (index) => index.eq('blockerUserId', userId))
			.take(100),
		coreLinksForUser(ctx, userId),
	])
	const friendships = await Promise.all(
		[...left, ...right].map(async (friendship) => {
			const otherId = friendship.userAId === userId ? friendship.userBId : friendship.userAId
			const other = await ctx.db.get(otherId)
			return other && !other.deletedAt
				? { friendshipId: friendship._id, user: publicUser(other), createdAt: friendship.createdAt }
				: null
		}),
	)
	const requests = async (items: typeof incoming, field: 'fromUserId' | 'toUserId') =>
		await Promise.all(
			items.map(async (request) => {
				const requestUser = await ctx.db.get(request[field])
				return {
					request: {
						_id: request._id,
						fromUserId: request.fromUserId,
						toUserId: request.toUserId,
						status: request.status,
						createdAt: request.createdAt,
						updatedAt: request.updatedAt,
					},
					user: requestUser && !requestUser.deletedAt ? publicUser(requestUser) : null,
				}
			}),
		)
	return {
		currentUser: publicUser(user, true, accountFields),
		profile: publicCurrentProfile(user, accountFields),
		friends: {
			friends: friendships.filter(
				(friendship): friendship is NonNullable<typeof friendship> => friendship !== null,
			),
			incoming: await requests(incoming, 'fromUserId'),
			outgoing: await requests(outgoing, 'toUserId'),
			blocks: await Promise.all(
				blocks.map(async (block) => {
					const blockedUser = await ctx.db.get(block.blockedUserId)
					return {
						blockId: block._id,
						user: blockedUser && !blockedUser.deletedAt ? publicUser(blockedUser) : null,
						createdAt: block.createdAt,
					}
				}),
			),
		},
		coreLinks,
		// Removed models stay explicit during the UI transition instead of being silently emulated.
		group: null,
		members: [],
		bans: [],
		pendingInvites: [],
		core: null,
	}
}

export async function liveScopeForUser(ctx: QueryCtx, userId: Id<'users'>) {
	const [left, right, memberUserIds] = await Promise.all([
		ctx.db
			.query('friendships')
			.withIndex('by_user_a', (index) => index.eq('userAId', userId))
			.take(100),
		ctx.db
			.query('friendships')
			.withIndex('by_user_b', (index) => index.eq('userBId', userId))
			.take(100),
		coreMemberUserIdsForUser(ctx, userId),
	])
	const friendUserIds = [...left, ...right].map((friendship) =>
		friendship.userAId === userId ? friendship.userBId : friendship.userAId,
	)
	return { userId, friendUserIds, memberUserIds }
}
