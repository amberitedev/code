import { v } from "convex/values";
import { internalQuery } from "./_generated/server";
import { coreById } from "./_socialRules";
import { liveScopeForCore, liveScopeForUser } from "./social";

export const desktopScope = internalQuery({
	args: { userId: v.id("users") },
	handler: async (ctx, args) => liveScopeForUser(ctx, args.userId),
});

export const coreScope = internalQuery({
	args: { coreId: v.string(), credentialHash: v.string() },
	handler: async (ctx, args) => {
		const core = await coreById(ctx, args.coreId);
		if (!core || core.realtimeCredentialHash !== args.credentialHash) return null;
		return liveScopeForCore(ctx, args.coreId);
	},
});

export const recipients = internalQuery({
	args: { kind: v.union(v.literal("desktop"), v.literal("core")), id: v.string(), credentialHash: v.optional(v.string()) },
	handler: async (ctx, args) => {
		if (args.kind === "desktop") return liveScopeForUser(ctx, args.id as never);
		if (!args.credentialHash) return null;
		const core = await coreById(ctx, args.id);
		if (!core || core.realtimeCredentialHash !== args.credentialHash) return null;
		return liveScopeForCore(ctx, args.id);
	},
});
