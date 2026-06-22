import { v } from 'convex/values'
import { query } from './_generated/server'
import type { Id } from './_generated/dataModel'
import type { QueryCtx } from './_generated/server'
import {
	bansByGroup,
	coreById,
	membersByGroup,
	membershipsByUser,
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
	const [left, right, incoming, outgoing, blocks, memberships] = await Promise.all([
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
		membershipsByUser(ctx, userId),
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
	const groupMembership = memberships[0] ?? null
	let group = null
	let members: ReturnType<typeof publicMembership>[] = []
	let bans: ReturnType<typeof publicBan>[] = []
	const pendingInvites = await Promise.all(
		(
			await ctx.db
				.query('friendGroupInvites')
				.withIndex('by_invitee_status', (q) =>
					q.eq('inviteeUserId', userId).eq('status', 'pending'),
				)
				.collect()
		).map(async (invite) => ({
			invite: publicInvite(invite),
			group: publicInviteGroup(await ctx.db.get(invite.friendGroupId as Id<'friendGroups'>)),
		})),
	)
	let core = null
	if (groupMembership) {
		const groupDoc = await ctx.db.get(groupMembership.friendGroupId as Id<'friendGroups'>)
		if (groupDoc) {
			core = publicCore(groupDoc.coreId ? await coreById(ctx, groupDoc.coreId) : null)
			group = {
				group: publicGroup(groupDoc),
				role: groupMembership.role,
				permissionPreset: groupMembership.permissionPreset,
				core,
			}
			members = await Promise.all(
				(await membersByGroup(ctx, groupMembership.friendGroupId)).map(async (member) => {
					const memberUser = await ctx.db.get(member.userId as Id<'users'>)
					return publicMembership(member, memberUser ? publicUser(memberUser) : null)
				}),
			)
			if (groupMembership.role === 'owner' || groupMembership.role === 'admin') {
				bans = await Promise.all(
					(await bansByGroup(ctx, groupMembership.friendGroupId)).map(async (ban) => {
						const bannedUser = await ctx.db.get(ban.userId as Id<'users'>)
						return publicBan(ban, bannedUser ? publicUser(bannedUser) : null)
					}),
				)
			}
		}
	}
	return {
		currentUser: publicUser(user, true),
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
		group,
		members,
		bans,
		pendingInvites,
		core,
	}
}

function publicCore(core: Awaited<ReturnType<typeof coreById>>) {
	if (!core) return null
	return {
		coreId: core.coreId,
		ownerUserId: core.ownerUserId,
		friendGroupId: core.friendGroupId,
		name: core.name,
		subdomain: core.subdomain,
		setupMode: core.setupMode,
		connectionUrl: core.connectionUrl,
		lastSeenAt: core.lastSeenAt,
		status: core.status,
	}
}

function publicGroup(group: NonNullable<Awaited<ReturnType<QueryCtx['db']['get']>>>) {
	return {
		_id: group._id,
		id: group._id,
		name: group.name,
		description: group.description,
		banner: group.banner,
		subdomain: group.subdomain,
		coreId: group.coreId,
		ownerUserId: group.ownerUserId,
		createdAt: group.createdAt,
		updatedAt: group.updatedAt,
	}
}

function publicMembership(member: any, user: ReturnType<typeof publicUser> | null) {
	return {
		_id: member._id,
		friendGroupId: member.friendGroupId,
		userId: member.userId,
		role: member.role,
		permissionPreset: member.permissionPreset,
		createdAt: member.createdAt,
		updatedAt: member.updatedAt,
		user,
	}
}

function publicBan(ban: any, user: ReturnType<typeof publicUser> | null) {
	return {
		_id: ban._id,
		friendGroupId: ban.friendGroupId,
		userId: ban.userId,
		bannedByUserId: ban.bannedByUserId,
		reason: ban.reason,
		createdAt: ban.createdAt,
		user,
	}
}

function publicInvite(invite: any) {
	return {
		_id: invite._id,
		friendGroupId: invite.friendGroupId,
		inviterUserId: invite.inviterUserId,
		inviteeUserId: invite.inviteeUserId,
		role: invite.role,
		status: invite.status,
		createdAt: invite.createdAt,
		expiresAt: invite.expiresAt,
		respondedAt: invite.respondedAt,
	}
}

function publicInviteGroup(group: Awaited<ReturnType<QueryCtx['db']['get']>>) {
	if (!group) return null
	return { id: group._id, name: (group as { name?: string }).name }
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
	const [left, right, memberships] = await Promise.all([
		ctx.db
			.query('friendships')
			.withIndex('by_user_a', (q) => q.eq('userAId', userId))
			.collect(),
		ctx.db
			.query('friendships')
			.withIndex('by_user_b', (q) => q.eq('userBId', userId))
			.collect(),
		membershipsByUser(ctx, userId),
	])
	const friendUserIds = [...left, ...right].map((friendship) =>
		friendship.userAId === userId ? friendship.userBId : friendship.userAId,
	)
	const group = memberships[0]
	const memberUserIds = group
		? (await membersByGroup(ctx, group.friendGroupId)).map((member) => member.userId)
		: []
	const groupDoc = group ? await ctx.db.get(group.friendGroupId as Id<'friendGroups'>) : null
	return { userId, friendUserIds, memberUserIds, coreId: groupDoc?.coreId ?? null }
}

export async function liveScopeForCore(ctx: QueryCtx, coreId: string) {
	const core = await coreById(ctx, coreId)
	if (!core?.friendGroupId) return null
	return {
		core,
		memberUserIds: (await membersByGroup(ctx, core.friendGroupId)).map((member) => member.userId),
	}
}
