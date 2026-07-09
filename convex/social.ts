import { v } from 'convex/values'
import { query } from './_generated/server'
import type { Id } from './_generated/dataModel'
import type { QueryCtx } from './_generated/server'
import { coreLinksForUser, coreMemberUserIdsForUser } from './coreList'
import {
	bansByGroup,
	coreById,
	currentAccountFields,
	membersByGroup,
	membershipsByUser,
	publicLegacyCore,
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
	const accountFields = await currentAccountFields(ctx, userId)
	const [left, right, incoming, outgoing, blocks, coreLinks, memberships, pendingInvites] =
		await Promise.all([
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
			membershipsByUser(ctx, userId),
			pendingGroupInvitesByUser(ctx, userId),
		])
	const membership = memberships[0] ?? null
	const activeGroup = membership ? await ctx.db.get(membership.friendGroupId as any) : null
	const activeGroupId = activeGroup?._id.toString()
	const [activeCore, activeMembers, activeBans, groupInvites] = await Promise.all([
		activeGroup?.coreId ? coreById(ctx, activeGroup.coreId) : Promise.resolve(null),
		activeGroupId ? publicMembersByGroup(ctx, activeGroupId) : Promise.resolve([]),
		activeGroupId && membership && canManageGroup(membership.role)
			? publicBansByGroup(ctx, activeGroupId)
			: Promise.resolve([]),
		Promise.all(
			pendingInvites.map(async (invite) => {
				const group = await ctx.db.get(invite.friendGroupId as any)
				return {
					invite,
					group: group ? publicFriendGroup(group) : null,
				}
			}),
		),
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
		group:
			activeGroup && membership
				? {
						group: publicFriendGroup(activeGroup),
						role: membership.role,
						permissionPreset: membership.permissionPreset,
						core: publicLegacyCore(activeCore),
					}
				: null,
		members: activeMembers,
		bans: activeBans,
		pendingInvites: groupInvites,
		core: publicLegacyCore(activeCore),
	}
}

async function publicMembersByGroup(ctx: QueryCtx, friendGroupId: string) {
	const members = await membersByGroup(ctx, friendGroupId)
	return await Promise.all(
		members.map(async (member) => {
			const memberUser = await ctx.db.get(member.userId as any)
			return { ...member, user: memberUser ? publicUser(memberUser) : null }
		}),
	)
}

async function publicBansByGroup(ctx: QueryCtx, friendGroupId: string) {
	const bans = await bansByGroup(ctx, friendGroupId)
	return await Promise.all(
		bans.map(async (ban) => {
			const bannedUser = await ctx.db.get(ban.userId as any)
			return { ...ban, user: bannedUser ? publicUser(bannedUser) : null }
		}),
	)
}

async function pendingGroupInvitesByUser(ctx: QueryCtx, userId: string) {
	const now = Date.now()
	const invites = await ctx.db
		.query('friendGroupInvites')
		.withIndex('by_invitee_status', (q) => q.eq('inviteeUserId', userId).eq('status', 'pending'))
		.collect()
	return invites.filter((invite) => invite.expiresAt > now)
}

function publicFriendGroup(group: any) {
	return { ...group, id: group._id }
}

function canManageGroup(role: string) {
	return role === 'owner' || role === 'admin'
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
