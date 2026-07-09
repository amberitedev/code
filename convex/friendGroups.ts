import { v } from 'convex/values'
import { mutation, query } from './_generated/server'
import {
	banByGroupUser,
	bansByGroup,
	coreById,
	coresByGroup,
	groupByCoreId,
	groupByIdOrSubdomain,
	membersByGroup,
	membershipByGroupUser,
	membershipsByUser,
	publicUser,
	publicLegacyCore,
	requireFriendGroupRole,
	requireSingleGroupMembership,
	requireSingleOwnedCore,
	resolveActor,
	roleRank,
	upsertCoreForFriendGroup,
} from './_socialRules'
const role = v.union(v.literal('owner'), v.literal('admin'), v.literal('member'))
const devActAs = { __actAs: v.optional(v.string()) }

export const listMyFriendGroups = query({
	args: { ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
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
							core: publicLegacyCore(core),
						}
					: null
			}),
		).then((groups) => groups.filter(Boolean))
	},
})

export const getFriendGroup = query({
	args: { friendGroupId: v.string(), ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
		await requireFriendGroupRole(ctx, userId, args.friendGroupId, ['owner', 'admin', 'member'])
		const group = await ctx.db.get(args.friendGroupId as any)
		return group
			? {
					group: { ...group, id: group._id },
					core: group.coreId ? publicLegacyCore(await coreById(ctx, group.coreId)) : null,
				}
			: null
	},
})

export const getPublicFriendGroupProfile = query({
	args: { idOrSubdomain: v.string(), ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
		const group = await groupByIdOrSubdomain(ctx, args.idOrSubdomain)
		if (!group) return null

		const friendGroupId = group._id.toString()
		const [members, viewerMembership, viewerBan, core] = await Promise.all([
			membersByGroup(ctx, friendGroupId),
			membershipByGroupUser(ctx, friendGroupId, userId),
			banByGroupUser(ctx, friendGroupId, userId),
			group.coreId ? coreById(ctx, group.coreId) : Promise.resolve(null),
		])
		const publicMembers = await Promise.all(
			members.map(async (member) => {
				const user = await ctx.db.get(member.userId as any)
				return { ...member, user: user ? publicUser(user) : null }
			}),
		)
		const canManage = viewerMembership
			? viewerMembership.role === 'owner' || viewerMembership.role === 'admin'
			: false

		return {
			group: { ...group, id: friendGroupId },
			core: publicLegacyCore(core),
			members: publicMembers,
			viewer: {
				isMember: Boolean(viewerMembership),
				role: viewerMembership?.role ?? null,
				permissionPreset: viewerMembership?.permissionPreset ?? null,
				canManage,
				isOwner: viewerMembership?.role === 'owner',
				banned: Boolean(viewerBan),
			},
			actions: {
				manage: canManage,
				leave: Boolean(viewerMembership && viewerMembership.role !== 'owner'),
				createInvite: canManage,
			},
		}
	},
})

export const listFriendGroupMembers = query({
	args: { friendGroupId: v.string(), ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
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
		...devActAs,
	},
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
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
		...devActAs,
	},
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
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
		...devActAs,
	},
	handler: async (ctx, args) => {
		const actorId = await resolveActor(ctx, args.__actAs)
		await requireFriendGroupRole(ctx, actorId, args.friendGroupId, ['owner', 'admin'])
		const actor = await membershipByGroupUser(ctx, args.friendGroupId, actorId)
		const member = await membershipByGroupUser(ctx, args.friendGroupId, args.userId)
		if (!member) throw new Error('member not found')
		if (member.role === 'owner' && args.role !== 'owner')
			throw new Error('owner role cannot be changed')
		if (member.role !== 'owner' && args.role === 'owner')
			throw new Error('owner role cannot be assigned')
		if (actor && actorId !== args.userId && roleRank(member.role) >= roleRank(actor.role))
			throw new Error('cannot manage a member of equal or higher rank')
		if (actor && args.role !== 'member' && roleRank(args.role) >= roleRank(actor.role))
			throw new Error('cannot promote a member to your own rank or higher')
		await ctx.db.patch(member._id, {
			role: args.role,
			permissionPreset: args.permissionPreset ?? member.permissionPreset ?? args.role,
			customPermissions: args.customPermissions,
			updatedAt: Date.now(),
		})
		return null
	},
})

export const removeMember = mutation({
	args: { friendGroupId: v.string(), userId: v.string(), ...devActAs },
	handler: async (ctx, args) => {
		const actorId = await resolveActor(ctx, args.__actAs)
		await requireFriendGroupRole(ctx, actorId, args.friendGroupId, ['owner', 'admin'])
		const actor = await membershipByGroupUser(ctx, args.friendGroupId, actorId)
		const member = await membershipByGroupUser(ctx, args.friendGroupId, args.userId)
		if (!member) return null
		if (member.role === 'owner') throw new Error('owner cannot be removed from their Core group')
		if (actor && actorId !== args.userId && roleRank(member.role) >= roleRank(actor.role))
			throw new Error('cannot remove a member of equal or higher rank')
		await ctx.db.delete(member._id)
		return null
	},
})

/**
 * Transfer ownership of the friend group (and its Core) to another member.
 * Only the current owner may do this. The previous owner is demoted to admin.
 */
export const transferOwnership = mutation({
	args: { friendGroupId: v.string(), userId: v.string(), ...devActAs },
	handler: async (ctx, args) => {
		const actorId = await resolveActor(ctx, args.__actAs)
		await requireFriendGroupRole(ctx, actorId, args.friendGroupId, ['owner'])
		if (actorId === args.userId) throw new Error('already the owner')
		const target = await membershipByGroupUser(ctx, args.friendGroupId, args.userId)
		if (!target) throw new Error('member not found')
		const current = await membershipByGroupUser(ctx, args.friendGroupId, actorId)
		const now = Date.now()
		if (current)
			await ctx.db.patch(current._id, { role: 'admin', permissionPreset: 'admin', updatedAt: now })
		await ctx.db.patch(target._id, { role: 'owner', permissionPreset: 'owner', updatedAt: now })
		await ctx.db.patch(args.friendGroupId as any, { ownerUserId: args.userId, updatedAt: now })
		const group = await ctx.db.get(args.friendGroupId as any)
		if (group?.coreId) {
			const core = await coreById(ctx, group.coreId)
			if (core) await ctx.db.patch(core._id, { ownerUserId: args.userId })
		}
		return null
	},
})

/**
 * Leave the friend group. The owner cannot leave while they still own the Core —
 * they must transfer ownership first (mirrors the "owner is permanent" invariant).
 */
export const leaveGroup = mutation({
	args: { friendGroupId: v.string(), ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
		const member = await membershipByGroupUser(ctx, args.friendGroupId, userId)
		if (!member) return null
		if (member.role === 'owner')
			throw new Error('transfer ownership before leaving your Core group')
		await ctx.db.delete(member._id)
		return null
	},
})

/**
 * Ban a user from the friend group: removes their membership and records a ban
 * so they cannot rejoin via invite until unbanned. Owner/admin only.
 */
export const banMember = mutation({
	args: { friendGroupId: v.string(), userId: v.string(), reason: v.optional(v.string()), ...devActAs },
	handler: async (ctx, args) => {
		const actorId = await resolveActor(ctx, args.__actAs)
		await requireFriendGroupRole(ctx, actorId, args.friendGroupId, ['owner', 'admin'])
		if (actorId === args.userId) throw new Error('cannot ban yourself')
		const actor = await membershipByGroupUser(ctx, args.friendGroupId, actorId)
		const member = await membershipByGroupUser(ctx, args.friendGroupId, args.userId)
		if (member?.role === 'owner') throw new Error('owner cannot be banned')
		if (actor && member && roleRank(member.role) >= roleRank(actor.role))
			throw new Error('cannot ban a member of equal or higher rank')
		if (member) await ctx.db.delete(member._id)
		const existing = await banByGroupUser(ctx, args.friendGroupId, args.userId)
		if (!existing)
			await ctx.db.insert('friendGroupBans', {
				friendGroupId: args.friendGroupId,
				userId: args.userId,
				bannedByUserId: actorId,
				reason: args.reason,
				createdAt: Date.now(),
			})
		return null
	},
})

export const unbanMember = mutation({
	args: { friendGroupId: v.string(), userId: v.string(), ...devActAs },
	handler: async (ctx, args) => {
		const actorId = await resolveActor(ctx, args.__actAs)
		await requireFriendGroupRole(ctx, actorId, args.friendGroupId, ['owner', 'admin'])
		const ban = await banByGroupUser(ctx, args.friendGroupId, args.userId)
		if (ban) await ctx.db.delete(ban._id)
		return null
	},
})

export const listFriendGroupBans = query({
	args: { friendGroupId: v.string(), ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
		await requireFriendGroupRole(ctx, userId, args.friendGroupId, ['owner', 'admin'])
		const bans = await bansByGroup(ctx, args.friendGroupId)
		return await Promise.all(
			bans.map(async (ban) => {
				const user = await ctx.db.get(ban.userId as any)
				return { ...ban, user: user ? publicUser(user) : null }
			}),
		)
	},
})

export const friendGroupCores = query({
	args: { friendGroupId: v.string(), ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs)
		await requireFriendGroupRole(ctx, userId, args.friendGroupId, ['owner', 'admin', 'member'])
		return (await coresByGroup(ctx, args.friendGroupId)).map(publicLegacyCore)
	},
})
