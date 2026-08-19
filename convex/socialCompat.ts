import { v } from 'convex/values'
import type { Doc, Id } from './_generated/dataModel'
import { mutation, query } from './_generated/server'
import type { MutationCtx, QueryCtx } from './_generated/server'
import {
	acceptedFriendship,
	blockByPair,
	canonicalPair,
	friendRequestByPair,
	minecraftUuid,
	requireUserId,
} from './_socialRules'
import {
	dismissNotificationByDedupeKey,
	notificationToLabrinth,
	upsertSocialNotification,
} from './_socialNotifications'

const friendValidator = v.object({
	id: v.string(),
	friend_id: v.string(),
	accepted: v.boolean(),
	created: v.string(),
})

const notificationValidator = v.object({
	id: v.string(),
	user_id: v.string(),
	type: v.union(v.string(), v.null()),
	title: v.string(),
	name: v.string(),
	text: v.string(),
	link: v.string(),
	read: v.boolean(),
	created: v.string(),
	actions: v.array(v.object({ title: v.string(), action_route: v.array(v.string()) })),
	body: v.object({
		type: v.string(),
		key: v.optional(v.string()),
		actor_user_id: v.optional(v.string()),
		shared_instance_id: v.optional(v.string()),
		shared_instance_name: v.optional(v.string()),
		shared_instance_icon: v.optional(v.null()),
		invited_by: v.optional(v.string()),
	}),
})

export const listFriends = query({
	args: {},
	returns: v.array(friendValidator),
	handler: async (ctx) => {
		const userId = await requireUserId(ctx)
		const [left, right, incoming, outgoing] = await Promise.all([
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
		])
		const accepted = await Promise.all(
			[...left, ...right].map(async (friendship) => {
				const [userA, userB] = await Promise.all([
					ctx.db.get(friendship.userAId),
					ctx.db.get(friendship.userBId),
				])
				if (!isActiveUser(userA) || !isActiveUser(userB)) return null
				const request = await acceptedRequestForPair(ctx, friendship.userAId, friendship.userBId)
				const requester = request?.fromUserId ?? friendship.userAId
				const accepter = request?.toUserId ?? friendship.userBId
				return {
					id: minecraftUuid(accepter === friendship.userAId ? userA : userB),
					friend_id: minecraftUuid(requester === friendship.userAId ? userA : userB),
					accepted: true,
					created: new Date(friendship.createdAt).toISOString(),
				}
			}),
		)
		const pending = await Promise.all(
			[...incoming, ...outgoing].map(async (request) => {
				const [from, to] = await Promise.all([
					ctx.db.get(request.fromUserId),
					ctx.db.get(request.toUserId),
				])
				if (!isActiveUser(from) || !isActiveUser(to)) return null
				return {
					id: minecraftUuid(to),
					friend_id: minecraftUuid(from),
					accepted: false,
					created: new Date(request.createdAt).toISOString(),
				}
			}),
		)
		return [...accepted, ...pending].filter(
			(entry): entry is NonNullable<typeof entry> => entry !== null,
		)
	},
})

export const addFriend = mutation({
	args: { idOrUsername: v.string() },
	returns: v.null(),
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		const target = await resolveUser(ctx, args.idOrUsername)
		if (!target || target._id === actorId || target.deletedAt) throw new Error('user not found')
		if (target.allowFriendRequests === false)
			throw new Error('friend requests are disabled for this user')
		await assertNeitherBlocked(ctx, actorId, target._id)
		if (await acceptedFriendship(ctx, actorId, target._id))
			throw new Error('you are already friends with this user')

		const incoming = await friendRequestByPair(ctx, target._id, actorId)
		if (incoming?.status === 'pending') {
			const now = Date.now()
			const [userAId, userBId] = canonicalPair(actorId, target._id)
			await ctx.db.patch(incoming._id, { status: 'accepted', updatedAt: now })
			if (!(await acceptedFriendship(ctx, actorId, target._id)))
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

		const existing = await friendRequestByPair(ctx, actorId, target._id)
		if (existing?.status === 'pending') throw new Error('you cannot accept your own friend request')
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

export const removeFriend = mutation({
	args: { idOrUsername: v.string() },
	returns: v.null(),
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		const target = await resolveUser(ctx, args.idOrUsername)
		if (!target) throw new Error('user not found')
		await removeRelationship(ctx, actorId, target._id)
		return null
	},
})

export const listBlocks = query({
	args: {},
	returns: v.array(v.string()),
	handler: async (ctx) => {
		const userId = await requireUserId(ctx)
		const blocks = await ctx.db
			.query('blockedUsers')
			.withIndex('by_blocker', (index) => index.eq('blockerUserId', userId))
			.take(100)
		const users = await Promise.all(blocks.map((block) => ctx.db.get(block.blockedUserId)))
		return users.filter(isActiveUser).map(minecraftUuid)
	},
})

export const blockUser = mutation({
	args: { idOrUsername: v.string() },
	returns: v.null(),
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		const target = await resolveUser(ctx, args.idOrUsername)
		if (!target || target._id === actorId || target.deletedAt) throw new Error('user not found')
		await removeRelationship(ctx, actorId, target._id)
		if (!(await blockByPair(ctx, actorId, target._id)))
			await ctx.db.insert('blockedUsers', {
				blockerUserId: actorId,
				blockedUserId: target._id,
				createdAt: Date.now(),
			})
		return null
	},
})

export const unblockUser = mutation({
	args: { idOrUsername: v.string() },
	returns: v.null(),
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		const target = await resolveUser(ctx, args.idOrUsername)
		if (target) {
			const block = await blockByPair(ctx, actorId, target._id)
			if (block) await ctx.db.delete(block._id)
		}
		return null
	},
})

export const listNotifications = query({
	args: { userId: v.optional(v.string()) },
	returns: v.array(notificationValidator),
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		const actor = await ctx.db.get(actorId)
		if (!actor) throw new Error('user not found')
		if (args.userId && args.userId !== minecraftUuid(actor)) throw new Error('not authorized')
		const rows = await ctx.db
			.query('socialNotifications')
			.withIndex('by_user', (index) => index.eq('userId', actorId))
			.order('desc')
			.take(100)
		return await Promise.all(
			rows
				.filter((row) => row.status !== 'dismissed')
				.map((row) => notificationToLabrinth(ctx, row)),
		)
	},
})

export const getNotifications = query({
	args: { ids: v.array(v.id('socialNotifications')) },
	returns: v.array(notificationValidator),
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		const rows = await Promise.all(args.ids.slice(0, 100).map((id) => ctx.db.get(id)))
		const owned = rows.filter((row): row is Doc<'socialNotifications'> =>
			Boolean(row && row.userId === actorId && row.status !== 'dismissed'),
		)
		return await Promise.all(owned.map((row) => notificationToLabrinth(ctx, row)))
	},
})

export const markNotificationsRead = mutation({
	args: { ids: v.array(v.id('socialNotifications')) },
	returns: v.null(),
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		for (const id of args.ids.slice(0, 100)) {
			const row = await ctx.db.get(id)
			if (row?.userId === actorId && row.status === 'unread')
				await ctx.db.patch(id, { status: 'read', updatedAt: Date.now() })
		}
		return null
	},
})

export const dismissNotifications = mutation({
	args: { ids: v.array(v.id('socialNotifications')) },
	returns: v.null(),
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		for (const id of args.ids.slice(0, 100)) {
			const row = await ctx.db.get(id)
			if (row?.userId === actorId)
				await ctx.db.patch(id, { status: 'dismissed', updatedAt: Date.now() })
		}
		return null
	},
})

async function resolveUser(ctx: QueryCtx | MutationCtx, value: string) {
	const input = value.trim().replace(/^@/, '')
	const uuid = input.replace(/-/g, '').toLowerCase()
	const byUuid = await ctx.db
		.query('users')
		.withIndex('by_minecraft_uuid', (index) => index.eq('minecraftUuid', uuid))
		.unique()
	if (byUuid) return byUuid
	return await ctx.db
		.query('users')
		.withIndex('by_normalized_username', (index) =>
			index.eq('normalizedUsername', input.toLowerCase()),
		)
		.unique()
}

function acceptedRequestForPair(ctx: QueryCtx, first: Id<'users'>, second: Id<'users'>) {
	return Promise.all([
		friendRequestByPair(ctx, first, second),
		friendRequestByPair(ctx, second, first),
	]).then(
		([forward, reverse]) =>
			[forward, reverse]
				.filter((request): request is NonNullable<typeof request> => request?.status === 'accepted')
				.sort((left, right) => right.updatedAt - left.updatedAt)[0] ?? null,
	)
}

async function assertNeitherBlocked(ctx: QueryCtx, first: Id<'users'>, second: Id<'users'>) {
	const [forward, reverse] = await Promise.all([
		blockByPair(ctx, first, second),
		blockByPair(ctx, second, first),
	])
	if (forward) throw new Error("you've blocked the other user")
	if (reverse) throw new Error("you've been blocked by the other user")
}

async function removeRelationship(ctx: MutationCtx, first: Id<'users'>, second: Id<'users'>) {
	const friendship = await acceptedFriendship(ctx, first, second)
	if (friendship) await ctx.db.delete(friendship._id)
	for (const request of await Promise.all([
		friendRequestByPair(ctx, first, second),
		friendRequestByPair(ctx, second, first),
	])) {
		if (request?.status !== 'pending') continue
		await ctx.db.patch(request._id, { status: 'canceled', updatedAt: Date.now() })
		await dismissNotificationByDedupeKey(ctx, request.toUserId, `friend-request:${request._id}`)
	}
}

function isActiveUser(user: Doc<'users'> | null): user is Doc<'users'> {
	return Boolean(user && !user.deletedAt && user.minecraftUuid)
}
