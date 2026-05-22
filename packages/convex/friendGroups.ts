import { v } from 'convex/values'
import { mutation, query } from './_generated/server'
import {
	coreById,
	coresByGroup,
	groupByCoreId,
	membersByGroup,
	membershipByGroupUser,
	membershipsByUser,
	publicUser,
	requireFriendGroupRole,
	requireSingleGroupMembership,
	requireSingleOwnedCore,
	requireUserId,
	upsertCoreForFriendGroup,
} from './_socialRules'
const role = v.union(v.literal('owner'), v.literal('admin'), v.literal('member'))

export const listMyFriendGroups = query({
	args: {},
	handler: async (ctx) => {
		const userId = await requireUserId(ctx)
		const memberships = await membershipsByUser(ctx, userId)
		return await Promise.all(
			memberships.map(async (membership) => {
				const group = await ctx.db.get(membership.friendGroupId as any)
				const core = group?.coreId ? await coreById(ctx, group.coreId) : null
				return group
					? {
							group: { ...group, id: group._id },
							role: membership.role,
							permissionPreset: membership.permissionPreset,
							core,
						}
					: null
			}),
		).then((groups) => groups.filter(Boolean))
	},
})

export const getFriendGroup = query({
	args: { friendGroupId: v.string() },
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)
		await requireFriendGroupRole(ctx, userId, args.friendGroupId, ['owner', 'admin', 'member'])
		const group = await ctx.db.get(args.friendGroupId as any)
		return group
			? {
					group: { ...group, id: group._id },
					core: group.coreId ? await coreById(ctx, group.coreId) : null,
				}
			: null
	},
})

export const listFriendGroupMembers = query({
	args: { friendGroupId: v.string() },
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)
		await requireFriendGroupRole(ctx, userId, args.friendGroupId, ['owner', 'admin', 'member'])
		const members = await membersByGroup(ctx, args.friendGroupId)
		return await Promise.all(
			members.map(async (member) => {
				const user = await ctx.db.get(member.userId as any)
				return { ...member, user: user ? publicUser(user) : null }
			}),
		)
	},
})

export const ensureCoreFriendGroup = mutation({
	args: {
		coreId: v.string(),
		connectionUrl: v.optional(v.string()),
		name: v.optional(v.string()),
		description: v.optional(v.string()),
		banner: v.optional(v.string()),
		subdomain: v.optional(v.string()),
		setupMode: v.optional(v.union(v.literal('remote'), v.literal('local'))),
	},
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)
		const now = Date.now()
		const existingGroup = await groupByCoreId(ctx, args.coreId)
		if (existingGroup) {
			const friendGroupId = existingGroup._id.toString()
			await requireFriendGroupRole(ctx, userId, friendGroupId, ['owner', 'admin'])
			await requireSingleGroupMembership(ctx, userId, friendGroupId)
			await upsertCoreForFriendGroup(
				ctx,
				args.coreId,
				existingGroup.ownerUserId,
				friendGroupId,
				args,
				now,
			)
			return { friendGroupId }
		}

		const existingCore = await coreById(ctx, args.coreId)
		if (existingCore && existingCore.ownerUserId !== userId)
			throw new Error('Core already belongs to another user')
		if (existingCore?.friendGroupId) throw new Error('Core already belongs to another friend group')
		await requireSingleGroupMembership(ctx, userId)
		await requireSingleOwnedCore(ctx, userId, args.coreId)

		const friendGroupId = await ctx.db.insert('friendGroups', {
			name: args.name ?? 'Amberite Friend Group',
			description: args.description,
			banner: args.banner,
			subdomain: args.subdomain,
			ownerUserId: userId,
			coreId: args.coreId,
			createdAt: now,
			updatedAt: now,
		})
		const id = friendGroupId.toString()
		await ctx.db.insert('friendGroupMembers', {
			friendGroupId: id,
			userId,
			role: 'owner',
			permissionPreset: 'owner',
			createdAt: now,
			updatedAt: now,
		})
		await upsertCoreForFriendGroup(ctx, args.coreId, userId, id, args, now)
		return { friendGroupId: id }
	},
})

export const updateFriendGroup = mutation({
	args: {
		friendGroupId: v.string(),
		name: v.optional(v.string()),
		description: v.optional(v.string()),
		banner: v.optional(v.string()),
		subdomain: v.optional(v.string()),
	},
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)
		await requireFriendGroupRole(ctx, userId, args.friendGroupId, ['owner', 'admin'])
		await ctx.db.patch(args.friendGroupId as any, {
			...(args.name !== undefined ? { name: args.name.trim() } : {}),
			...(args.description !== undefined ? { description: args.description.trim() } : {}),
			...(args.banner !== undefined ? { banner: args.banner.trim() } : {}),
			...(args.subdomain !== undefined ? { subdomain: args.subdomain.trim().toLowerCase() } : {}),
			updatedAt: Date.now(),
		})
		return null
	},
})

export const updateMemberRole = mutation({
	args: {
		friendGroupId: v.string(),
		userId: v.string(),
		role,
		permissionPreset: v.optional(v.string()),
		customPermissions: v.optional(v.any()),
	},
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		await requireFriendGroupRole(ctx, actorId, args.friendGroupId, ['owner', 'admin'])
		const member = await membershipByGroupUser(ctx, args.friendGroupId, args.userId)
		if (!member) throw new Error('member not found')
		if (member.role === 'owner' && args.role !== 'owner')
			throw new Error('owner role cannot be changed')
		if (member.role !== 'owner' && args.role === 'owner')
			throw new Error('owner role cannot be assigned')
		await ctx.db.patch(member._id, {
			role: args.role,
			permissionPreset: args.permissionPreset,
			customPermissions: args.customPermissions,
			updatedAt: Date.now(),
		})
		return null
	},
})

export const removeMember = mutation({
	args: { friendGroupId: v.string(), userId: v.string() },
	handler: async (ctx, args) => {
		const actorId = await requireUserId(ctx)
		await requireFriendGroupRole(ctx, actorId, args.friendGroupId, ['owner', 'admin'])
		const member = await membershipByGroupUser(ctx, args.friendGroupId, args.userId)
		if (!member) return null
		if (member.role === 'owner') throw new Error('owner cannot be removed from their Core group')
		await ctx.db.delete(member._id)
		return null
	},
})

export const friendGroupCores = query({
	args: { friendGroupId: v.string() },
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx)
		await requireFriendGroupRole(ctx, userId, args.friendGroupId, ['owner', 'admin', 'member'])
		return await coresByGroup(ctx, args.friendGroupId)
	},
})
