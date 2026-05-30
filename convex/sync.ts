import { getAuthUserId } from "@convex-dev/auth/server";
import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import type { MutationCtx, QueryCtx } from "./_generated/server";

export const registerSyncedProfile = mutation({
	args: {
		friendGroupId: v.string(),
		coreId: v.string(),
		coreInstanceId: v.string(),
		clientProfileId: v.optional(v.string()),
		name: v.string(),
		gameVersion: v.optional(v.string()),
		loader: v.optional(v.string()),
		syncEnabled: v.optional(v.boolean()),
	},
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);
		await requireFriendGroupRole(ctx, userId, args.friendGroupId, ["owner", "admin"]);
		const now = Date.now();
		const existing = await syncedProfileByCoreInstance(ctx, args.coreId, args.coreInstanceId);
		const value = { ...args, syncEnabled: args.syncEnabled ?? true, status: "active" as const, updatedAt: now };
		if (existing) {
			await ctx.db.patch(existing._id, value);
			return { profileId: existing._id };
		}
		const profileId = await ctx.db.insert("syncedProfiles", { ...value, createdAt: now });
		return { profileId };
	},
});

export const listServerProfiles = query({
	args: { friendGroupId: v.string() },
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);
		await requireFriendGroupRole(ctx, userId, args.friendGroupId, ["owner", "admin", "member"]);
		return await syncedProfilesByGroup(ctx, args.friendGroupId);
	},
});

export const publishProfileSnapshot = mutation({
	args: {
		profileId: v.id("syncedProfiles"),
		manifest: v.any(),
		clientOnlyManifest: v.optional(v.any()),
		serverManifest: v.optional(v.any()),
		notes: v.optional(v.string()),
	},
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);
		const profile = await ctx.db.get(args.profileId);
		if (!profile) throw new Error("synced profile not found");
		await requireFriendGroupRole(ctx, userId, profile.friendGroupId, ["owner", "admin", "member"]);
		const now = Date.now();
		const snapshotId = await ctx.db.insert("profileSnapshots", {
			profileId: args.profileId,
			authorUserId: userId,
			manifest: args.manifest,
			clientOnlyManifest: args.clientOnlyManifest,
			serverManifest: args.serverManifest,
			notes: args.notes,
			createdAt: now,
		});
		await ctx.db.insert("modSyncEvents", {
			profileId: args.profileId,
			snapshotId,
			authorUserId: userId,
			status: "planned",
			message: "Snapshot stored. Diff/apply pipeline is intentionally deferred.",
			createdAt: now,
		});
		await ctx.db.patch(args.profileId, { updatedAt: now });
		return { snapshotId };
	},
});

export const listProfileSnapshots = query({
	args: { profileId: v.id("syncedProfiles") },
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);
		const profile = await ctx.db.get(args.profileId);
		if (!profile) throw new Error("synced profile not found");
		await requireFriendGroupRole(ctx, userId, profile.friendGroupId, ["owner", "admin", "member"]);
		return await snapshotsByProfile(ctx, args.profileId);
	},
});

export const listModSyncEvents = query({
	args: { profileId: v.id("syncedProfiles") },
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);
		const profile = await ctx.db.get(args.profileId);
		if (!profile) throw new Error("synced profile not found");
		await requireFriendGroupRole(ctx, userId, profile.friendGroupId, ["owner", "admin", "member"]);
		return await syncEventsByProfile(ctx, args.profileId);
	},
});

async function requireUserId(ctx: QueryCtx | MutationCtx) {
	const userId = await getAuthUserId(ctx);
	if (userId === null) throw new Error("not authenticated");
	return userId;
}

async function requireFriendGroupRole(ctx: QueryCtx | MutationCtx, userId: string, friendGroupId: string, allowed: Array<"owner" | "admin" | "member">) {
	const membership = await membershipByGroupUser(ctx, friendGroupId, userId);
	if (!membership || !allowed.includes(membership.role)) throw new Error("not authorized for friend group");
}

function syncedProfileByCoreInstance(ctx: QueryCtx | MutationCtx, coreId: string, coreInstanceId: string) {
	return ctx.db.query("syncedProfiles").withIndex("by_core_instance", (q) => q.eq("coreId", coreId).eq("coreInstanceId", coreInstanceId)).unique();
}
function syncedProfilesByGroup(ctx: QueryCtx, friendGroupId: string) {
	return ctx.db.query("syncedProfiles").withIndex("by_group", (q) => q.eq("friendGroupId", friendGroupId)).collect();
}
function snapshotsByProfile(ctx: QueryCtx, profileId: string) {
	return ctx.db.query("profileSnapshots").withIndex("by_profile", (q) => q.eq("profileId", profileId)).order("desc").take(50);
}
function syncEventsByProfile(ctx: QueryCtx, profileId: string) {
	return ctx.db.query("modSyncEvents").withIndex("by_profile", (q) => q.eq("profileId", profileId)).order("desc").take(50);
}
function membershipByGroupUser(ctx: QueryCtx | MutationCtx, friendGroupId: string, userId: string) {
	return ctx.db.query("friendGroupMembers").withIndex("by_group_user", (q) => q.eq("friendGroupId", friendGroupId).eq("userId", userId)).unique();
}
