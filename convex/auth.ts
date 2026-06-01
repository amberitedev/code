import MicrosoftEntraID from "@auth/core/providers/microsoft-entra-id";
import { convexAuth, getAuthUserId } from "@convex-dev/auth/server";
import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import { resolveActor } from "./_socialRules";

export const { auth, signIn, signOut, store, isAuthenticated } = convexAuth({
	providers: [
		MicrosoftEntraID({
			issuer: process.env.AUTH_MICROSOFT_ENTRA_ID_ISSUER ?? "https://login.microsoftonline.com/consumers/v2.0",
			authorization: {
				params: { scope: "openid profile email XboxLive.signin" },
			},
		}),
	],
});

export const currentUser = query({
	args: { __actAs: v.optional(v.string()) },
	handler: async (ctx, args) => {
		const userId =
			args.__actAs && process.env.AMBERITE_DEV_MODE === "true"
				? (args.__actAs as any)
				: await getAuthUserId(ctx);
		if (userId === null) return null;
		const user = await ctx.db.get(userId);
		return user ? { ...user, userId } : null;
	},
});

export const setUsername = mutation({
	args: {
		username: v.string(),
		displayName: v.optional(v.string()),
		__actAs: v.optional(v.string()),
	},
	handler: async (ctx, args) => {
		const userId = await resolveActor(ctx, args.__actAs);

		const username = args.username.trim();
		if (!/^[a-zA-Z0-9_]{3,24}$/.test(username)) {
			throw new Error("username must be 3-24 letters, numbers, or underscores");
		}

		const normalizedUsername = username.toLowerCase();
		const existing = await ctx.db
			.query("users")
			.withIndex("by_normalized_username", (q) => q.eq("normalizedUsername", normalizedUsername))
			.unique();
		if (existing && existing._id !== userId) throw new Error("username is already taken");

		const user = await ctx.db.get(userId);
		const now = Date.now();
		await ctx.db.patch(userId, {
			amberiteUserId: user?.amberiteUserId ?? crypto.randomUUID(),
			username,
			normalizedUsername,
			displayName: args.displayName?.trim() || username,
			onboardedAt: user?.onboardedAt ?? now,
		});

		return { userId, username };
	},
});
