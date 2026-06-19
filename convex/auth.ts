import MicrosoftEntraID from "@auth/core/providers/microsoft-entra-id";
import { ConvexCredentials } from "@convex-dev/auth/providers/ConvexCredentials";
import { convexAuth, getAuthUserId } from "@convex-dev/auth/server";
import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import { publicUser, resolveActor } from "./_socialRules";

interface MinecraftProfile {
	id: string;
	name: string;
}

function normalizeUuid(raw: string): string {
	const stripped = raw.replace(/-/g, "");
	if (stripped.length !== 32) return raw;
	return [
		stripped.slice(0, 8),
		stripped.slice(8, 12),
		stripped.slice(12, 16),
		stripped.slice(16, 20),
		stripped.slice(20, 32),
	].join("-");
}

async function verifyMinecraftAccessToken(token: string): Promise<MinecraftProfile | null> {
	const response = await fetch("https://api.minecraftservices.com/minecraft/profile", {
		headers: { Authorization: `Bearer ${token}` },
	});
	if (!response.ok) return null;
	return (await response.json()) as MinecraftProfile;
}

export const { auth, signIn, signOut, store, isAuthenticated } = convexAuth({
	providers: [
		MicrosoftEntraID({
			issuer: process.env.AUTH_MICROSOFT_ENTRA_ID_ISSUER ?? "https://login.microsoftonline.com/consumers/v2.0",
			authorization: {
				params: { scope: "openid profile email XboxLive.signin" },
			},
		}),
		ConvexCredentials({
			id: "minecraft-token",
			authorize: async (credentials, ctx) => {
				const minecraftAccessToken = credentials.minecraftAccessToken as string | undefined;
				if (!minecraftAccessToken) return null;

				const profile = await verifyMinecraftAccessToken(minecraftAccessToken);
				if (!profile) return null;

				const minecraftUuid = normalizeUuid(profile.id);
				const gamertag = profile.name;

				const existing = await ctx.db
					.query("linkedMicrosoftAccounts")
					.withIndex("by_microsoft_account", (q) =>
						q.eq("microsoftAccountId", `minecraft:${minecraftUuid}`),
					)
					.unique();

				if (existing) {
					const user = await ctx.db.get(existing.amberiteUserId as any);
					if (user) {
						await ctx.db.patch(existing._id, { gamertag, minecraftUuid });
						return { userId: existing.amberiteUserId as any };
					}
				}

				const now = Date.now();
				const userId = await ctx.db.insert("users", {
					amberiteUserId: crypto.randomUUID(),
					displayName: gamertag,
					username: gamertag,
					normalizedUsername: gamertag.toLowerCase(),
					onboardedAt: now,
				});

				await ctx.db.insert("linkedMicrosoftAccounts", {
					amberiteUserId: userId,
					microsoftAccountId: `minecraft:${minecraftUuid}`,
					gamertag,
					minecraftUuid,
					linkedAt: now,
				});

				return { userId };
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
		return user ? publicUser({ ...user, _id: userId }) : null;
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
