import { v } from 'convex/values'
import type { Doc, Id } from './_generated/dataModel'
import { mutation, query } from './_generated/server'
import type { MutationCtx, QueryCtx } from './_generated/server'
import { canonicalPair, requireUserId } from './_socialRules'
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
	type: v.string(),
	title: v.string(),
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
		const id = userId.toString()
		const [left, right, incoming, outgoing] = await Promise.all([
			ctx.db
				.query('friendships')
				.withIndex('by_user_a', (q) => q.eq('userAId', id))
				.take(100),
			ctx.db
				.query('friendships')
				.withIndex('by_user_b', (q) => q.eq('userBId', id))
				.take(100),
			ctx.db
				.query('friendRequests')
				.withIndex('by_to_status', (q) => q.eq('toUserId', id).eq('status', 'pending'))
				.take(100),
			ctx.db
				.query('friendRequests')
				.withIndex('by_from_status', (q) => q.eq('fromUserId', id).eq('status', 'pending'))
				.take(100),
		])
		return [
			...[...left, ...right].map((friendship) => ({
				id: friendship.userAId,
				friend_id: friendship.userBId,
				accepted: true,
				created: new Date(friendship.createdAt).toISOString(),
			})),
			...[...incoming, ...outgoing].map((request) => ({
				id: request.toUserId,
				friend_id: request.fromUserId,
				accepted: false,
				created: new Date(request.createdAt).toISOString(),
			})),
		]
	},
})

export const addFriend = mutation({
	args: { idOrUsername: v.string() },
	returns: v.null(),
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		const target = await resolveUser(ctx, args.idOrUsername)
		if (!target || target._id === actorId || target.deletedAt) throw new Error('user not found')
		if (target.allowFriendRequests === false) throw new Error('user has friend requests disabled')
		await assertNeitherBlocked(ctx, actorId, target._id)
		if (await friendship(ctx, actorId, target._id)) return null

		const incoming = await requestByPair(ctx, target._id, actorId)
		if (incoming?.status === 'pending') {
			const now = Date.now()
			const [userAId, userBId] = canonicalPair(actorId, target._id)
			await ctx.db.patch(incoming._id, { status: 'accepted', updatedAt: now })
			if (!(await friendship(ctx, actorId, target._id)))
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

		const existing = await requestByPair(ctx, actorId, target._id)
		if (existing?.status === 'pending') return null
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
		if (!target) return null
		const accepted = await friendship(ctx, actorId, target._id)
		if (accepted) await ctx.db.delete(accepted._id)
		for (const request of await Promise.all([
			requestByPair(ctx, actorId, target._id),
			requestByPair(ctx, target._id, actorId),
		])) {
			if (request?.status === 'pending') {
				await ctx.db.patch(request._id, { status: 'canceled', updatedAt: Date.now() })
				await dismissNotificationByDedupeKey(
					ctx,
					request.toUserId === actorId ? actorId : target._id,
					`friend-request:${request._id}`,
				)
			}
		}
		return null
	},
})

export const listBlocks = query({
	args: {},
	returns: v.array(v.string()),
	handler: async (ctx) => {
		const userId = await requireUserId(ctx)
		return (
			await ctx.db
				.query('blockedUsers')
				.withIndex('by_blocker', (q) => q.eq('blockerUserId', userId))
				.take(100)
		).map((block) => block.blockedUserId)
	},
})

export const blockUser = mutation({
	args: { idOrUsername: v.string() },
	returns: v.null(),
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		const target = await resolveUser(ctx, args.idOrUsername)
		if (!target || target._id === actorId) throw new Error('user not found')
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
		if (args.userId && args.userId !== actorId) throw new Error('not authorized')
		const rows = await ctx.db
			.query('socialNotifications')
			.withIndex('by_user', (q) => q.eq('userId', actorId))
			.order('desc')
			.take(100)
		const active = rows.filter((row) => row.status !== 'dismissed')
		return await Promise.all(active.map((row) => notificationToLabrinth(ctx, row)))
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

async function resolveUser(ctx: QueryCtx | MutationCtx, idOrUsername: string) {
	const id = ctx.db.normalizeId('users', idOrUsername)
	if (id) return await ctx.db.get(id)
	return await ctx.db
		.query('users')
		.withIndex('by_verified_minecraft_handle', (q) =>
			q.eq('normalizedVerifiedMinecraftHandle', idOrUsername.trim().toLowerCase()),
		)
		.unique()
}

function requestByPair(ctx: QueryCtx | MutationCtx, from: Id<'users'>, to: Id<'users'>) {
	return ctx.db
		.query('friendRequests')
		.withIndex('by_from_to', (q) => q.eq('fromUserId', from).eq('toUserId', to))
		.order('desc')
		.first()
}

function blockByPair(ctx: QueryCtx | MutationCtx, blocker: Id<'users'>, blocked: Id<'users'>) {
	return ctx.db
		.query('blockedUsers')
		.withIndex('by_blocker_blocked', (q) =>
			q.eq('blockerUserId', blocker).eq('blockedUserId', blocked),
		)
		.unique()
}

function friendship(ctx: QueryCtx | MutationCtx, first: Id<'users'>, second: Id<'users'>) {
	const [userAId, userBId] = canonicalPair(first, second)
	return ctx.db
		.query('friendships')
		.withIndex('by_pair', (q) => q.eq('userAId', userAId).eq('userBId', userBId))
		.unique()
}

async function assertNeitherBlocked(
	ctx: QueryCtx | MutationCtx,
	first: Id<'users'>,
	second: Id<'users'>,
) {
	if ((await blockByPair(ctx, first, second)) || (await blockByPair(ctx, second, first)))
		throw new Error('user not found')
}

async function removeRelationship(ctx: MutationCtx, first: Id<'users'>, second: Id<'users'>) {
	const accepted = await friendship(ctx, first, second)
	if (accepted) await ctx.db.delete(accepted._id)
	for (const request of await Promise.all([
		requestByPair(ctx, first, second),
		requestByPair(ctx, second, first),
	])) {
		if (request?.status === 'pending') {
			await ctx.db.patch(request._id, { status: 'canceled', updatedAt: Date.now() })
			await dismissNotificationByDedupeKey(
				ctx,
				request.toUserId === first ? first : second,
				`friend-request:${request._id}`,
			)
		}
	}
}
