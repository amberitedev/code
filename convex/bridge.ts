import { v } from "convex/values";
import { internalQuery } from "./_generated/server";
import { liveScopeForUser } from "./social";

export const desktopScope = internalQuery({
	args: { userId: v.id("users") },
	handler: async (ctx, args) => liveScopeForUser(ctx, args.userId),
});

export const recipients = internalQuery({
	args: { kind: v.literal("desktop"), id: v.string() },
	handler: async (ctx, args) => liveScopeForUser(ctx, args.id as never),
});
