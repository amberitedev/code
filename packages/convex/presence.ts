import { getAuthUserId } from "@convex-dev/auth/server";
import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import type { MutationCtx, QueryCtx } from "./_generated/server";

export const registerCore = mutation({
	args: {
		coreId: v.string(),
		ownerUserId: v.string(),
		friendGroupId: v.optional(v.string()),
		connectionUrl: v.optional(v.string()),
		status: v.optional(v.string()),
		metadata: v.optional(v.any()),
	},
	returns: v.object({ coreId: v.string() }),
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);
		if (args.ownerUserId !== userId) throw new Error("cannot register a Core for another user");
		if (args.friendGroupId) await requireFriendGroupRole(ctx, userId, args.friendGroupId, ["owner", "admin"]);

		const now = Date.now();
		const existing = await ctx.db
			.query("cores")
			.withIndex("by_core_id", (q) => q.eq("coreId", args.coreId))
			.unique();

		if (existing) {
			await requireCoreRole(ctx, userId, args.coreId, ["owner", "admin"]);
			await ctx.db.patch(existing._id, {
				ownerUserId: args.ownerUserId,
				friendGroupId: args.friendGroupId,
				connectionUrl: args.connectionUrl,
				status: args.status,
				metadata: args.metadata,
				lastSeenAt: now,
			});
		} else {
			await ctx.db.insert("cores", { ...args, lastSeenAt: now });
		}

		return { coreId: args.coreId };
	},
});

export const heartbeatCore = mutation({
	args: { coreId: v.string(), status: v.optional(v.string()), metadata: v.optional(v.any()) },
	returns: v.null(),
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);
		await requireCoreRole(ctx, userId, args.coreId, ["owner", "admin"]);

		const existing = await ctx.db
			.query("cores")
			.withIndex("by_core_id", (q) => q.eq("coreId", args.coreId))
			.unique();
		if (!existing) return null;
		await ctx.db.patch(existing._id, {
			lastSeenAt: Date.now(),
			status: args.status,
			metadata: args.metadata,
		});
		return null;
	},
});

export const corePresence = query({
	args: { coreId: v.string() },
	returns: v.union(
		v.null(),
		v.object({
			coreId: v.string(),
			ownerUserId: v.string(),
			friendGroupId: v.optional(v.string()),
			connectionUrl: v.optional(v.string()),
			lastSeenAt: v.number(),
			status: v.optional(v.string()),
			metadata: v.optional(v.any()),
		}),
	),
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);
		const core = await ctx.db
			.query("cores")
			.withIndex("by_core_id", (q) => q.eq("coreId", args.coreId))
			.unique();
		if (!core) return null;
		await requireCoreAccess(ctx, userId, core);
		return core;
	},
});

export const registerPairingCore = mutation({
	args: {
		code: v.string(),
		coreId: v.string(),
		connectionUrl: v.optional(v.string()),
		metadata: v.optional(v.any()),
		ttlMs: v.optional(v.number()),
	},
	returns: v.object({ coreId: v.string(), code: v.string() }),
	handler: async (ctx, args) => {
		const now = Date.now();
		const existing = await ctx.db
			.query("pairingCores")
			.withIndex("by_core_id", (q) => q.eq("coreId", args.coreId))
			.unique();
		const value = {
			code: args.code,
			coreId: args.coreId,
			connectionUrl: args.connectionUrl,
			status: "waiting" as const,
			metadata: args.metadata,
			createdAt: now,
			expiresAt: now + (args.ttlMs ?? 10 * 60 * 1000),
		};

		if (existing) await ctx.db.patch(existing._id, value);
		else await ctx.db.insert("pairingCores", value);
		return { coreId: args.coreId, code: args.code };
	},
});

export const claimPairingCore = mutation({
	args: { code: v.string() },
	returns: v.union(
		v.null(),
		v.object({ coreId: v.string(), connectionUrl: v.optional(v.string()) }),
	),
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);
		const now = Date.now();
		const pairing = await ctx.db
			.query("pairingCores")
			.withIndex("by_code", (q) => q.eq("code", args.code))
			.unique();
		if (!pairing || pairing.status !== "waiting" || pairing.expiresAt <= now) return null;

		const friendGroup = await getOrCreateDefaultFriendGroup(ctx, userId, pairing.coreId);
		const existingCore = await ctx.db
			.query("cores")
			.withIndex("by_core_id", (q) => q.eq("coreId", pairing.coreId))
			.unique();
		const coreValue = {
			coreId: pairing.coreId,
			ownerUserId: userId,
			friendGroupId: friendGroup,
			connectionUrl: pairing.connectionUrl,
			lastSeenAt: now,
			status: "paired",
			metadata: pairing.metadata,
		};

		if (existingCore) await ctx.db.patch(existingCore._id, coreValue);
		else await ctx.db.insert("cores", coreValue);

		await ctx.db.patch(pairing._id, {
			status: "claimed",
			ownerUserId: userId,
			claimedAt: now,
		});
		return { coreId: pairing.coreId, connectionUrl: pairing.connectionUrl };
	},
});

async function requireUserId(ctx: QueryCtx | MutationCtx): Promise<string> {
	const userId = await getAuthUserId(ctx);
	if (userId === null) throw new Error("not authenticated");
	return userId;
}

async function requireCoreRole(
	ctx: QueryCtx | MutationCtx,
	userId: string,
	coreId: string,
	allowedRoles: Array<"owner" | "admin" | "member">,
): Promise<void> {
	const core = await ctx.db
		.query("cores")
		.withIndex("by_core_id", (q) => q.eq("coreId", coreId))
		.unique();
	if (!core) throw new Error("Core not found");
	await requireCoreAccess(ctx, userId, core, allowedRoles);
}

async function requireCoreAccess(
	ctx: QueryCtx | MutationCtx,
	userId: string,
	core: { ownerUserId: string; friendGroupId?: string },
	allowedRoles: Array<"owner" | "admin" | "member"> = ["owner", "admin", "member"],
): Promise<void> {
	if (core.ownerUserId === userId && allowedRoles.includes("owner")) return;
	if (!core.friendGroupId) throw new Error("not authorized for Core");
	await requireFriendGroupRole(ctx, userId, core.friendGroupId, allowedRoles);
}

async function requireFriendGroupRole(
	ctx: QueryCtx | MutationCtx,
	userId: string,
	friendGroupId: string,
	allowedRoles: Array<"owner" | "admin" | "member">,
): Promise<void> {
	const membership = await ctx.db
		.query("friendGroupMembers")
		.withIndex("by_group_user", (q) => q.eq("friendGroupId", friendGroupId).eq("userId", userId))
		.unique();
	if (!membership || !allowedRoles.includes(membership.role)) throw new Error("not authorized for friend group");
}

async function getOrCreateDefaultFriendGroup(ctx: MutationCtx, userId: string, coreId: string): Promise<string> {
	const existing = await ctx.db
		.query("friendGroups")
		.withIndex("by_core_id", (q) => q.eq("coreId", coreId))
		.unique();
	if (existing) return existing._id.toString();

	const now = Date.now();
	const friendGroupId = await ctx.db.insert("friendGroups", {
		ownerUserId: userId,
		coreId,
		createdAt: now,
	});
	const friendGroupIdString = friendGroupId.toString();
	await ctx.db.insert("friendGroupMembers", {
		friendGroupId: friendGroupIdString,
		userId,
		role: "owner",
		createdAt: now,
	});
	return friendGroupIdString;
}
