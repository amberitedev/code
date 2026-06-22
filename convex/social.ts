import { v } from "convex/values";
import { query } from "./_generated/server";
import type { Id } from "./_generated/dataModel";
import type { QueryCtx } from "./_generated/server";
import {
	bansByGroup,
	coreById,
	membersByGroup,
	membershipsByUser,
	publicUser,
	resolveActor,
} from "./_socialRules";

const devActAs = { __actAs: v.optional(v.string()) };

/** The single durable social subscription for the desktop shell. */
export const sessionState = query({
	args: { ...devActAs },
	handler: async (ctx, args) => sessionStateForUser(ctx, await resolveActor(ctx, args.__actAs)),
});

export async function sessionStateForUser(ctx: QueryCtx, userId: Id<"users">) {
	const user = await ctx.db.get(userId);
	if (!user) throw new Error("user not found");
	const [left, right, incoming, outgoing, blocks, memberships] = await Promise.all([
		ctx.db.query("friendships").withIndex("by_user_a", (q) => q.eq("userAId", userId)).collect(),
		ctx.db.query("friendships").withIndex("by_user_b", (q) => q.eq("userBId", userId)).collect(),
		ctx.db.query("friendRequests").withIndex("by_to_status", (q) => q.eq("toUserId", userId).eq("status", "pending")).collect(),
		ctx.db.query("friendRequests").withIndex("by_from_status", (q) => q.eq("fromUserId", userId).eq("status", "pending")).collect(),
		ctx.db.query("blockedUsers").withIndex("by_blocker", (q) => q.eq("blockerUserId", userId)).collect(),
		membershipsByUser(ctx, userId),
	]);
	const friendships = await Promise.all([...left, ...right].map(async (friendship) => {
		const otherId = friendship.userAId === userId ? friendship.userBId : friendship.userAId;
		const other = await ctx.db.get(otherId as Id<"users">);
		return other ? { friendshipId: friendship._id, user: publicUser(other), createdAt: friendship.createdAt } : null;
	}));
	const requests = async (items: typeof incoming, field: "fromUserId" | "toUserId") => Promise.all(items.map(async (request) => {
		const requestUser = await ctx.db.get(request[field] as Id<"users">);
		return { request, user: requestUser ? publicUser(requestUser) : null };
	}));
	const groupMembership = memberships[0] ?? null;
	let group: unknown = null;
	let members: unknown[] = [];
	let bans: unknown[] = [];
	let pendingInvites: unknown[] = [];
	let core: unknown = null;
	if (groupMembership) {
		const groupDoc = await ctx.db.get(groupMembership.friendGroupId as Id<"friendGroups">);
		if (groupDoc) {
			core = groupDoc.coreId ? await coreById(ctx, groupDoc.coreId) : null;
			group = { group: { ...groupDoc, id: groupDoc._id }, role: groupMembership.role, permissionPreset: groupMembership.permissionPreset, core };
			members = await Promise.all((await membersByGroup(ctx, groupMembership.friendGroupId)).map(async (member) => {
				const memberUser = await ctx.db.get(member.userId as Id<"users">);
				return { ...member, user: memberUser ? publicUser(memberUser) : null };
			}));
			if (groupMembership.role === "owner" || groupMembership.role === "admin") {
				bans = await Promise.all((await bansByGroup(ctx, groupMembership.friendGroupId)).map(async (ban) => {
					const bannedUser = await ctx.db.get(ban.userId as Id<"users">);
					return { ...ban, user: bannedUser ? publicUser(bannedUser) : null };
				}));
			}
		}
		pendingInvites = await Promise.all((await ctx.db.query("friendGroupInvites")
			.withIndex("by_invitee_status", (q) => q.eq("inviteeUserId", userId).eq("status", "pending"))
			.collect()).map(async (invite) => ({ invite, group: await ctx.db.get(invite.friendGroupId as Id<"friendGroups">) })));
	}
	return {
		currentUser: publicUser(user),
		friends: {
			friends: friendships.filter((friendship): friendship is NonNullable<typeof friendship> => friendship !== null),
			incoming: await requests(incoming, "fromUserId"),
			outgoing: await requests(outgoing, "toUserId"),
			blocks: await Promise.all(blocks.map(async (block) => {
				const blockedUser = await ctx.db.get(block.blockedUserId as Id<"users">);
				return { blockId: block._id, user: blockedUser ? publicUser(blockedUser) : null, createdAt: block.createdAt };
			})),
		},
		group,
		members,
		bans,
		pendingInvites,
		core,
	};
}

export async function liveScopeForUser(ctx: QueryCtx, userId: Id<"users">) {
	const [left, right, memberships] = await Promise.all([
		ctx.db.query("friendships").withIndex("by_user_a", (q) => q.eq("userAId", userId)).collect(),
		ctx.db.query("friendships").withIndex("by_user_b", (q) => q.eq("userBId", userId)).collect(),
		membershipsByUser(ctx, userId),
	]);
	const friendUserIds = [...left, ...right].map((friendship) => friendship.userAId === userId ? friendship.userBId : friendship.userAId);
	const group = memberships[0];
	const memberUserIds = group ? (await membersByGroup(ctx, group.friendGroupId)).map((member) => member.userId) : [];
	const groupDoc = group ? await ctx.db.get(group.friendGroupId as Id<"friendGroups">) : null;
	return { userId, friendUserIds, memberUserIds, coreId: groupDoc?.coreId ?? null };
}

export async function liveScopeForCore(ctx: QueryCtx, coreId: string) {
	const core = await coreById(ctx, coreId);
	if (!core?.friendGroupId) return null;
	return { core, memberUserIds: (await membersByGroup(ctx, core.friendGroupId)).map((member) => member.userId) };
}
