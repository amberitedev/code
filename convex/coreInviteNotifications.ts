import { v } from 'convex/values'
import { mutation, query } from './_generated/server'
import { resolveActor } from './_socialRules'

export const notify = mutation({
	args: { recipientUserId: v.string(), coreId: v.string(), coreUrl: v.string(), inviteId: v.string(), expiresAt: v.number() },
	handler: async (ctx, args) => {
		const actor = await resolveActor(ctx)
		if (!actor) throw new Error('not authenticated')
		const existing = await ctx.db.query('coreInviteNotifications').withIndex('by_invite', (q) => q.eq('inviteId', args.inviteId)).unique()
		if (existing) return { notificationId: existing._id.toString() }
		const notificationId = await ctx.db.insert('coreInviteNotifications', { ...args, createdAt: Date.now() })
		return { notificationId: notificationId.toString() }
	},
})

export const mine = query({
	args: {},
	handler: async (ctx) => {
		const userId = await resolveActor(ctx)
		return await ctx.db.query('coreInviteNotifications').withIndex('by_recipient_expiry', (q) => q.eq('recipientUserId', userId)).collect().then((items) => items.filter((item) => item.expiresAt > Date.now()))
	},
})

export const markRead = mutation({
	args: { notificationId: v.id('coreInviteNotifications') },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx)
		const notification = await ctx.db.get(args.notificationId)
		if (!notification || notification.recipientUserId !== userId) throw new Error('notification not found')
		await ctx.db.patch(args.notificationId, { readAt: Date.now() })
		return null
	},
})
