/**
 * sync.ts — synced server profiles for a friend group.
 *
 * Beyond basic registration/snapshots this owns two user-facing features:
 *  - Selective visibility: a profile can be shown to everyone in the group,
 *    to specific roles, or to a hand-picked set of members (private servers).
 *  - Automatic whitelists: when enabled, the eligible members' linked Minecraft
 *    accounts are resolved into a whitelist the Core can apply.
 *
 * All functions resolve the acting user through `resolveActor` so the dev
 * identity override (`__actAs`) works in dev and tests; production uses real auth.
 *
 * Key functions: registerSyncedProfile:30, listServerProfiles:54 (visibility-gated),
 * updateSyncedProfileSettings:74, getProfileWhitelist:104, publishProfileSnapshot:130.
 */
import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import type { MutationCtx, QueryCtx } from "./_generated/server";
import {
	coreById,
	membersByGroup,
	membershipByGroupUser,
	requireFriendGroupRole,
	resolveActor,
} from "./_socialRules";

const devActAs = { __actAs: v.optional(v.string()) };
const friendGroupRole = v.union(v.literal("owner"), v.literal("admin"), v.literal("member"));
const profileVisibility = v.union(v.literal("everyone"), v.literal("roles"), v.literal("custom"));
const whitelistScope = v.union(v.literal("viewers"), v.literal("roles"), v.literal("custom"));

type Role = "owner" | "admin" | "member";

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
		visibility: v.optional(profileVisibility),
		...devActAs,
	},
	handler: async (ctx, args) => {
		const { __actAs, ...rest } = args;
		const userId = await resolveActor(ctx, __actAs);
		await requireFriendGroupRole(ctx, userId, args.friendGroupId, ["owner", "admin"]);
		await requireCoreInGroup(ctx, args.coreId, args.friendGroupId);
		const now = Date.now();
		const existing = await syncedProfileByCoreInstance(ctx, args.coreId, args.coreInstanceId);
		const value = {
			...rest,
			syncEnabled: args.syncEnabled ?? true,
			visibility: args.visibility ?? ("everyone" as const),
			status: "active" as const,
			updatedAt: now,
		};
		if (existing) {
			await ctx.db.patch(existing._id, value);
			return { profileId: existing._id };
		}
		const profileId = await ctx.db.insert("syncedProfiles", { ...value, createdAt: now });
		return { profileId };
	},
});

export const listServerProfiles = query({
	args: { friendGroupId: v.string(), ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs);
		const membership = await membershipByGroupUser(ctx, args.friendGroupId, userId);
		if (!membership) throw new Error("not authorized for friend group");
		const role = membership.role as Role;
		const profiles = await syncedProfilesByGroup(ctx, args.friendGroupId);
		const manages = role === "owner" || role === "admin";
		return profiles
			.filter((profile) => canViewProfile(role, userId, profile))
			.map((profile) => ({ ...profile, viewerCanManage: manages }));
	},
});

export const updateSyncedProfileSettings = mutation({
	args: {
		profileId: v.id("syncedProfiles"),
		name: v.optional(v.string()),
		syncEnabled: v.optional(v.boolean()),
		status: v.optional(v.union(v.literal("active"), v.literal("paused"), v.literal("archived"))),
		visibility: v.optional(profileVisibility),
		visibilityRoles: v.optional(v.array(friendGroupRole)),
		visibilityUserIds: v.optional(v.array(v.string())),
		autoWhitelist: v.optional(v.boolean()),
		whitelistScope: v.optional(whitelistScope),
		whitelistRoles: v.optional(v.array(friendGroupRole)),
		whitelistUserIds: v.optional(v.array(v.string())),
		...devActAs,
	},
	handler: async (ctx, args) => {
		const { profileId, __actAs, ...patch } = args;
		const userId = await resolveActor(ctx, __actAs);
		const profile = await ctx.db.get(profileId);
		if (!profile) throw new Error("synced profile not found");
		await requireFriendGroupRole(ctx, userId, profile.friendGroupId, ["owner", "admin"]);
		if (patch.visibilityUserIds) {
			await requireUsersInGroup(ctx, profile.friendGroupId, patch.visibilityUserIds);
		}
		if (patch.whitelistUserIds) {
			await requireUsersInGroup(ctx, profile.friendGroupId, patch.whitelistUserIds);
		}
		const clean = Object.fromEntries(Object.entries(patch).filter(([, value]) => value !== undefined));
		await ctx.db.patch(profileId, { ...clean, updatedAt: Date.now() });
		return { ok: true };
	},
});

export const getProfileWhitelist = query({
	args: { profileId: v.id("syncedProfiles"), ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs);
		const profile = await ctx.db.get(args.profileId);
		if (!profile) throw new Error("synced profile not found");
		await requireFriendGroupRole(ctx, userId, profile.friendGroupId, ["owner", "admin"]);
		const members = await membersByGroup(ctx, profile.friendGroupId);
		const eligible = members.filter((member) =>
			isWhitelisted(member.role as Role, member.userId, profile),
		);
		const entries = await Promise.all(
			eligible.map(async (member) => {
				const user = await ctx.db.get(member.userId as never);
				const accounts = user?.amberiteUserId
					? await linkedAccountsByUser(ctx, user.amberiteUserId)
					: [];
				return {
					userId: member.userId,
					role: member.role,
					displayName: user?.displayName ?? user?.username ?? null,
					accounts: accounts.map((account) => ({
						gamertag: account.gamertag ?? null,
						minecraftUuid: account.minecraftUuid ?? null,
					})),
				};
			}),
		);
		return { autoWhitelist: profile.autoWhitelist ?? false, entries };
	},
});

export const publishProfileSnapshot = mutation({
	args: {
		profileId: v.id("syncedProfiles"),
		manifest: v.any(),
		clientOnlyManifest: v.optional(v.any()),
		serverManifest: v.optional(v.any()),
		notes: v.optional(v.string()),
		...devActAs,
	},
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs);
		const profile = await ctx.db.get(args.profileId);
		if (!profile) throw new Error("synced profile not found");
		await requireFriendGroupRole(ctx, userId, profile.friendGroupId, ["owner", "admin", "member"]);
		const membership = await membershipByGroupUser(ctx, profile.friendGroupId, userId);
		if (!membership || !canViewProfile(membership.role as Role, userId, profile)) {
			throw new Error("not authorized for synced profile");
		}
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
	args: { profileId: v.id("syncedProfiles"), ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs);
		const profile = await ctx.db.get(args.profileId);
		if (!profile) throw new Error("synced profile not found");
		await requireFriendGroupRole(ctx, userId, profile.friendGroupId, ["owner", "admin", "member"]);
		const membership = await membershipByGroupUser(ctx, profile.friendGroupId, userId);
		if (!membership || !canViewProfile(membership.role as Role, userId, profile)) {
			throw new Error("not authorized for synced profile");
		}
		return await snapshotsByProfile(ctx, args.profileId);
	},
});

export const listModSyncEvents = query({
	args: { profileId: v.id("syncedProfiles"), ...devActAs },
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs);
		const profile = await ctx.db.get(args.profileId);
		if (!profile) throw new Error("synced profile not found");
		await requireFriendGroupRole(ctx, userId, profile.friendGroupId, ["owner", "admin", "member"]);
		const membership = await membershipByGroupUser(ctx, profile.friendGroupId, userId);
		if (!membership || !canViewProfile(membership.role as Role, userId, profile)) {
			throw new Error("not authorized for synced profile");
		}
		return await syncEventsByProfile(ctx, args.profileId);
	},
});

function canViewProfile(role: Role, userId: string, profile: any): boolean {
	if (role === "owner" || role === "admin") return true;
	const visibility = profile.visibility ?? "everyone";
	if (visibility === "everyone") return true;
	if (visibility === "roles") return (profile.visibilityRoles ?? []).includes(role);
	if (visibility === "custom") return (profile.visibilityUserIds ?? []).includes(userId);
	return false;
}

function isWhitelisted(role: Role, userId: string, profile: any): boolean {
	if (!(profile.autoWhitelist ?? false)) return false;
	const scope = profile.whitelistScope ?? "viewers";
	if (scope === "viewers") return canViewProfile(role, userId, profile);
	if (scope === "roles") return (profile.whitelistRoles ?? []).includes(role);
	if (scope === "custom") return (profile.whitelistUserIds ?? []).includes(userId);
	return false;
}

async function requireCoreInGroup(
	ctx: QueryCtx | MutationCtx,
	coreId: string,
	friendGroupId: string,
): Promise<void> {
	const core = await coreById(ctx, coreId);
	if (!core || core.friendGroupId !== friendGroupId) {
		throw new Error("Core does not belong to friend group");
	}
}

async function requireUsersInGroup(
	ctx: QueryCtx | MutationCtx,
	friendGroupId: string,
	userIds: string[],
): Promise<void> {
	for (const userId of userIds) {
		const membership = await membershipByGroupUser(ctx, friendGroupId, userId);
		if (!membership) throw new Error("user is not a member of this friend group");
	}
}

function syncedProfileByCoreInstance(ctx: QueryCtx | MutationCtx, coreId: string, coreInstanceId: string) {
	return ctx.db
		.query("syncedProfiles")
		.withIndex("by_core_instance", (q) => q.eq("coreId", coreId).eq("coreInstanceId", coreInstanceId))
		.unique();
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
function linkedAccountsByUser(ctx: QueryCtx | MutationCtx, amberiteUserId: string) {
	return ctx.db
		.query("linkedMicrosoftAccounts")
		.withIndex("by_amberite_user", (q) => q.eq("amberiteUserId", amberiteUserId))
		.collect();
}
