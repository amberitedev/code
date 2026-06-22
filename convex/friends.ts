import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import type { MutationCtx, QueryCtx } from "./_generated/server";
import { resolveActor } from "./_socialRules";

/** Optional dev-only acting-user override, honoured only when AMBERITE_DEV_MODE is set. */
const devActAs = { __actAs: v.optional(v.string()) };
const FRIEND_REQUEST_NOTIFICATION_CLAIM_MS = 60_000;

export const searchUsers = query({
	args: { query: v.string(), ...devActAs },
	handler: async (ctx, args) => {
		const viewerId = await resolveActor(ctx, args.__actAs);
		const term = args.query.trim();
		if (term.length < 2) return [];
		const normalizedTerm = term.toLowerCase();
		const upperBound = `${normalizedTerm}\uffff`;
		const users = await ctx.db
			.query("users")
			.withIndex("by_normalized_username", (q) =>
				q.gte("normalizedUsername", normalizedTerm).lt("normalizedUsername", upperBound),
			)
			.take(8);
		return users
			.filter((user) => {
				if (user._id === viewerId) return false;
				return user.normalizedUsername?.startsWith(normalizedTerm);
			})
			.map((user) => publicUser(user));
	},
});

export const friendsList = query({
	args: { ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs);
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
			return {
				friendshipId: friendship._id,
				user: user ? publicUser(user) : null,
				createdAt: friendship.createdAt,
			};
		}));
		const incomingWithUser = await Promise.all(incoming.map(async (request) => {
			const user = await ctx.db.get(request.fromUserId as any);
			return { request, user: user ? publicUser(user) : null };
		}));
		const outgoingWithUser = await Promise.all(outgoing.map(async (request) => {
			const user = await ctx.db.get(request.toUserId as any);
			return { request, user: user ? publicUser(user) : null };
		}));
		const blocksWithUser = await Promise.all(blocks.map(async (block) => {
			const user = await ctx.db.get(block.blockedUserId as any);
			return {
				blockId: block._id,
				user: user ? publicUser(user) : null,
				createdAt: block.createdAt,
			};
		}));
		return { friends: friends.filter((friend) => friend.user), incoming: incomingWithUser, outgoing: outgoingWithUser, blocks: blocksWithUser };
	},
});

export const sendFriendRequest = mutation({
	args: { targetUserId: v.optional(v.id("users")), friendCode: v.optional(v.string()), username: v.optional(v.string()), message: v.optional(v.string()), ...devActAs },
	handler: async (ctx, args) => {
		const fromUserId = await resolveActor(ctx, args.__actAs);
		const target = await resolveTargetUser(ctx, args);
		if (!target || target._id === fromUserId) throw new Error("friend target not found");
		await assertNotBlocked(ctx, fromUserId, target._id);
		const [userAId, userBId] = canonicalPair(fromUserId, target._id);
		const friendship = await friendshipByPair(ctx, userAId, userBId);
		if (friendship) return { requestId: null, status: "already_friends" };
		const existing = await friendRequestByPair(ctx, fromUserId, target._id);
		if (existing?.status === "pending") return { requestId: existing._id, status: "pending" };
		const incoming = await friendRequestByPair(ctx, target._id, fromUserId);
		if (incoming?.status === "pending") return { requestId: incoming._id, status: "incoming_pending" };
		const now = Date.now();
		if (existing) {
			await ctx.db.patch(existing._id, {
				status: "pending",
				message: args.message,
				updatedAt: now,
				notificationClaimedAt: undefined,
				notificationClaimExpiresAt: undefined,
				notificationDeliveredAt: undefined,
			});
			return { requestId: existing._id, status: "pending" };
		}
		const requestId = await ctx.db.insert("friendRequests", { fromUserId, toUserId: target._id, status: "pending", message: args.message, createdAt: now, updatedAt: now });
		return { requestId, status: "pending" };
	},
});

export const claimFriendRequestNotifications = mutation({
	args: { ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs);
		const now = Date.now();
		const requests = await friendRequestsTo(ctx, userId);
		const claimable = requests.filter((request) =>
			!request.notificationDeliveredAt &&
			(!request.notificationClaimExpiresAt || request.notificationClaimExpiresAt <= now),
		);
		const expiresAt = now + FRIEND_REQUEST_NOTIFICATION_CLAIM_MS;
		return await Promise.all(claimable.map(async (request) => {
			await ctx.db.patch(request._id, {
				notificationClaimedAt: now,
				notificationClaimExpiresAt: expiresAt,
			});
			const user = await ctx.db.get(request.fromUserId as any);
			return { request: { ...request, notificationClaimedAt: now, notificationClaimExpiresAt: expiresAt }, user: user ? publicUser(user) : null };
		}));
	},
});

export const acknowledgeFriendRequestNotification = mutation({
	args: { requestId: v.id("friendRequests"), ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs);
		const request = await ctx.db.get(args.requestId);
		if (!request || request.toUserId !== userId) throw new Error("friend request not found");
		if (request.status === "pending" && !request.notificationDeliveredAt) {
			await ctx.db.patch(request._id, {
				notificationDeliveredAt: Date.now(),
				notificationClaimExpiresAt: undefined,
			});
		}
		return null;
	},
});

export const respondFriendRequest = mutation({
	args: { requestId: v.id("friendRequests"), accept: v.boolean(), ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs);
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

export const cancelFriendRequest = mutation({
	args: { requestId: v.id("friendRequests"), ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs);
		const request = await ctx.db.get(args.requestId);
		if (!request || request.fromUserId !== userId || request.status !== "pending") throw new Error("friend request not found");
		await ctx.db.patch(args.requestId, { status: "canceled", updatedAt: Date.now() });
		return null;
	},
});

export const removeFriend = mutation({
	args: { userId: v.string(), ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs);
		const [userAId, userBId] = canonicalPair(userId, args.userId);
		const friendship = await friendshipByPair(ctx, userAId, userBId);
		if (friendship) await ctx.db.delete(friendship._id);
		return null;
	},
});

export const blockUser = mutation({
	args: { userId: v.string(), ...devActAs },
	handler: async (ctx, args) => {
		const blockerUserId = await resolveActor(ctx, args.__actAs);
		if (blockerUserId === args.userId) throw new Error("cannot block yourself");
		await removeFriendship(ctx, blockerUserId, args.userId);
		const existing = await blockByPair(ctx, blockerUserId, args.userId);
		if (!existing) await ctx.db.insert("blockedUsers", { blockerUserId, blockedUserId: args.userId, createdAt: Date.now() });
		return null;
	},
});

export const unblockUser = mutation({
	args: { userId: v.string(), ...devActAs },
	handler: async (ctx, args) => {
		const blockerUserId = await resolveActor(ctx, args.__actAs);
		const block = await blockByPair(ctx, blockerUserId, args.userId);
		if (block) await ctx.db.delete(block._id);
		return null;
	},
});

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
	return ctx.db.query("friendRequests").withIndex("by_from_to", (q) => q.eq("fromUserId", fromUserId).eq("toUserId", toUserId)).order("desc").first();
}

function blockByPair(ctx: QueryCtx | MutationCtx, blockerUserId: string, blockedUserId: string) {
	return ctx.db.query("blockedUsers").withIndex("by_blocker_blocked", (q) => q.eq("blockerUserId", blockerUserId).eq("blockedUserId", blockedUserId)).unique();
}
function canonicalPair(a: string, b: string): [string, string] {
	return a < b ? [a, b] : [b, a];
}

const publicUser = (user: any) => ({ userId: user._id, username: user.username, displayName: user.displayName, image: user.image, friendCode: user.friendCode });
