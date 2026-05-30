import { getAuthUserId } from "@convex-dev/auth/server";
import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import type { MutationCtx, QueryCtx } from "./_generated/server";

export const ensureSocialProfile = mutation({
	args: {},
	handler: async (ctx) => {
		const userId = await requireUserId(ctx);
		const user = await ctx.db.get(userId);
		if (!user) throw new Error("user not found");
		const patch: Record<string, unknown> = {};
		if (!user.friendCode) patch.friendCode = await createFriendCode(ctx);
		if (!user.amberiteUserId) patch.amberiteUserId = crypto.randomUUID();
		if (Object.keys(patch).length > 0) await ctx.db.patch(userId, patch);
		return publicUser({ ...user, ...patch, _id: userId });
	},
});

export const searchUsers = query({
	args: { query: v.string() },
	handler: async (ctx, args) => {
		const viewerId = await requireUserId(ctx);
		const term = args.query.trim();
		if (term.length < 3) return [];
		const byCode = await userByFriendCode(ctx, term);
		const byName = await userByUsername(ctx, term);
		return [byCode, byName]
			.filter((user, index, users) => user && user._id !== viewerId && users.findIndex((u) => u?._id === user._id) === index)
			.map((user) => publicUser(user!));
	},
});

export const friendsList = query({
	args: {},
	handler: async (ctx) => {
		const userId = await requireUserId(ctx);
		const [left, right, incoming, outgoing, blocks] = await Promise.all([
			friendshipsByUserA(ctx, userId),
			friendshipsByUserB(ctx, userId),
			friendRequestsTo(ctx, userId),
			friendRequestsFrom(ctx, userId),
			blocksByUser(ctx, userId),
		]);
		const friendships = [...left, ...right];
		const friends = await Promise.all(friendships.map(async (friendship) => {
			const otherId = friendship.userAId === userId ? friendship.userBId : friendship.userAId;
			const user = await ctx.db.get(otherId as any);
			return { friendshipId: friendship._id, user: user ? publicUser(user) : null, createdAt: friendship.createdAt };
		}));
		return { friends: friends.filter((friend) => friend.user), incoming, outgoing, blocks };
	},
});

export const sendFriendRequest = mutation({
	args: { targetUserId: v.optional(v.id("users")), friendCode: v.optional(v.string()), username: v.optional(v.string()), message: v.optional(v.string()) },
	handler: async (ctx, args) => {
		const fromUserId = await requireUserId(ctx);
		const target = await resolveTargetUser(ctx, args);
		if (!target || target._id === fromUserId) throw new Error("friend target not found");
		await assertNotBlocked(ctx, fromUserId, target._id);
		const [userAId, userBId] = canonicalPair(fromUserId, target._id);
		const friendship = await friendshipByPair(ctx, userAId, userBId);
		if (friendship) return { requestId: null, status: "already_friends" };
		const existing = await friendRequestByPair(ctx, fromUserId, target._id);
		if (existing?.status === "pending") return { requestId: existing._id, status: "pending" };
		const now = Date.now();
		const requestId = await ctx.db.insert("friendRequests", { fromUserId, toUserId: target._id, status: "pending", message: args.message, createdAt: now, updatedAt: now });
		return { requestId, status: "pending" };
	},
});

export const respondFriendRequest = mutation({
	args: { requestId: v.id("friendRequests"), accept: v.boolean() },
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);
		const request = await ctx.db.get(args.requestId);
		if (!request || request.toUserId !== userId || request.status !== "pending") throw new Error("friend request not found");
		const now = Date.now();
		await ctx.db.patch(args.requestId, { status: args.accept ? "accepted" : "declined", updatedAt: now });
		if (args.accept) {
			const [userAId, userBId] = canonicalPair(request.fromUserId, request.toUserId);
			const existing = await friendshipByPair(ctx, userAId, userBId);
			if (!existing) await ctx.db.insert("friendships", { userAId, userBId, createdAt: now });
		}
		return null;
	},
});

export const removeFriend = mutation({
	args: { userId: v.string() },
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);
		const [userAId, userBId] = canonicalPair(userId, args.userId);
		const friendship = await friendshipByPair(ctx, userAId, userBId);
		if (friendship) await ctx.db.delete(friendship._id);
		return null;
	},
});

export const blockUser = mutation({
	args: { userId: v.string() },
	handler: async (ctx, args) => {
		const blockerUserId = await requireUserId(ctx);
		if (blockerUserId === args.userId) throw new Error("cannot block yourself");
		await removeFriendship(ctx, blockerUserId, args.userId);
		const existing = await blockByPair(ctx, blockerUserId, args.userId);
		if (!existing) await ctx.db.insert("blockedUsers", { blockerUserId, blockedUserId: args.userId, createdAt: Date.now() });
		return null;
	},
});

export const unblockUser = mutation({
	args: { userId: v.string() },
	handler: async (ctx, args) => {
		const blockerUserId = await requireUserId(ctx);
		const block = await blockByPair(ctx, blockerUserId, args.userId);
		if (block) await ctx.db.delete(block._id);
		return null;
	},
});

async function requireUserId(ctx: QueryCtx | MutationCtx) {
	const userId = await getAuthUserId(ctx);
	if (userId === null) throw new Error("not authenticated");
	return userId;
}

async function createFriendCode(ctx: MutationCtx): Promise<string> {
	for (let i = 0; i < 10; i++) {
		const code = `AMB-${Math.random().toString(36).slice(2, 8).toUpperCase()}`;
		const existing = await userByFriendCode(ctx, code);
		if (!existing) return code;
	}
	throw new Error("could not allocate friend code");
}

async function resolveTargetUser(ctx: MutationCtx, args: { targetUserId?: string; friendCode?: string; username?: string }) {
	if (args.targetUserId) return await ctx.db.get(args.targetUserId as any);
	if (args.friendCode) return await userByFriendCode(ctx, args.friendCode);
	if (args.username) return await userByUsername(ctx, args.username);
	return null;
}

async function assertNotBlocked(ctx: QueryCtx, a: string, b: string) {
	const [ab, ba] = await Promise.all([
		blockByPair(ctx, a, b),
		blockByPair(ctx, b, a),
	]);
	if (ab || ba) throw new Error("blocked users cannot interact");
}

async function removeFriendship(ctx: MutationCtx, a: string, b: string) {
	const [userAId, userBId] = canonicalPair(a, b);
	const friendship = await friendshipByPair(ctx, userAId, userBId);
	if (friendship) await ctx.db.delete(friendship._id);
}

function userByFriendCode(ctx: QueryCtx | MutationCtx, code: string) {
	return ctx.db.query("users").withIndex("by_friend_code", (q) => q.eq("friendCode", code.trim().toUpperCase())).unique();
}
function userByUsername(ctx: QueryCtx | MutationCtx, username: string) {
	return ctx.db.query("users").withIndex("by_normalized_username", (q) => q.eq("normalizedUsername", username.trim().toLowerCase())).unique();
}
function friendshipsByUserA(ctx: QueryCtx, userId: string) {
	return ctx.db.query("friendships").withIndex("by_user_a", (q) => q.eq("userAId", userId)).collect();
}
function friendshipsByUserB(ctx: QueryCtx, userId: string) {
	return ctx.db.query("friendships").withIndex("by_user_b", (q) => q.eq("userBId", userId)).collect();
}
function friendRequestsTo(ctx: QueryCtx, userId: string) {
	return ctx.db.query("friendRequests").withIndex("by_to_status", (q) => q.eq("toUserId", userId).eq("status", "pending")).collect();
}
function friendRequestsFrom(ctx: QueryCtx, userId: string) {
	return ctx.db.query("friendRequests").withIndex("by_from_status", (q) => q.eq("fromUserId", userId).eq("status", "pending")).collect();
}
function blocksByUser(ctx: QueryCtx, userId: string) {
	return ctx.db.query("blockedUsers").withIndex("by_blocker", (q) => q.eq("blockerUserId", userId)).collect();
}
function friendshipByPair(ctx: QueryCtx | MutationCtx, userAId: string, userBId: string) {
	return ctx.db.query("friendships").withIndex("by_pair", (q) => q.eq("userAId", userAId).eq("userBId", userBId)).unique();
}

function friendRequestByPair(ctx: QueryCtx | MutationCtx, fromUserId: string, toUserId: string) {
	return ctx.db.query("friendRequests").withIndex("by_from_to", (q) => q.eq("fromUserId", fromUserId).eq("toUserId", toUserId)).unique();
}

function blockByPair(ctx: QueryCtx | MutationCtx, blockerUserId: string, blockedUserId: string) {
	return ctx.db.query("blockedUsers").withIndex("by_blocker_blocked", (q) => q.eq("blockerUserId", blockerUserId).eq("blockedUserId", blockedUserId)).unique();
}

function canonicalPair(a: string, b: string): [string, string] {
	return a < b ? [a, b] : [b, a];
}

const publicUser = (user: any) => ({ userId: user._id, username: user.username, displayName: user.displayName, image: user.image, friendCode: user.friendCode });
