import MicrosoftEntraID from "@auth/core/providers/microsoft-entra-id";
import { ConvexCredentials } from "@convex-dev/auth/providers/ConvexCredentials";
import {
	convexAuth,
	createAccount,
	getAuthUserId,
	retrieveAccount,
} from "@convex-dev/auth/server";
import { v } from "convex/values";
import { internal } from "./_generated/api";
import { internalMutation, mutation, query } from "./_generated/server";
import { publicUser, requireUserId } from "./_socialRules";

interface MinecraftProfile {
	id: string;
	name: string;
}

const MINECRAFT_TOKEN_PROVIDER_ID = "minecraft-token";
const INVALID_ACCOUNT_ID = "InvalidAccountId";
const DEV_PERSONA_ID_PATTERN = /^[a-z0-9_-]{1,32}$/;

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

function devPersonaId(credentials: Record<string, unknown>): string | null {
	const raw = credentials.devPersonaId;
	if (raw === undefined || raw === null || raw === "") return null;
	if (process.env.AMBERITE_DEV_MODE !== "true") {
		throw new Error("devPersonaId is only accepted in Amberite dev mode");
	}
	if (typeof raw !== "string" || !DEV_PERSONA_ID_PATTERN.test(raw)) {
		throw new Error("devPersonaId must be 1-32 lowercase letters, numbers, underscores, or hyphens");
	}
	return raw;
}

function personaAccountId(minecraftUuid: string, personaId: string | null): string {
	const baseAccountId = `minecraft:${minecraftUuid}`;
	return personaId ? `${baseAccountId}:dev:${personaId}` : baseAccountId;
}

function personaProfile(gamertag: string, accountId: string, personaId: string | null) {
	if (!personaId) {
		return {
			amberiteUserId: accountId,
			displayName: gamertag,
			username: gamertag,
			normalizedUsername: gamertag.toLowerCase(),
			onboardedAt: Date.now(),
		};
	}

	const label = personaId
		.split(/[-_]/g)
		.filter(Boolean)
		.map((part) => part.charAt(0).toUpperCase() + part.slice(1))
		.join(" ");
	const username = personaId.replace(/-/g, "_");
	return {
		amberiteUserId: accountId,
		displayName: label || gamertag,
		username,
		normalizedUsername: username.toLowerCase(),
		onboardedAt: Date.now(),
	};
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
			id: MINECRAFT_TOKEN_PROVIDER_ID,
			authorize: async (credentials, ctx) => {
				const minecraftAccessToken = credentials.minecraftAccessToken as string | undefined;
				if (!minecraftAccessToken) return null;

				const profile = await verifyMinecraftAccessToken(minecraftAccessToken);
				if (!profile) return null;

				const minecraftUuid = normalizeUuid(profile.id);
				const gamertag = profile.name;
				const personaId = devPersonaId(credentials);
				const accountId = personaAccountId(minecraftUuid, personaId);

				const existing = await retrieveAccount(ctx, {
					provider: MINECRAFT_TOKEN_PROVIDER_ID,
					account: { id: accountId },
				}).catch((error) => {
					if (error instanceof Error && error.message === INVALID_ACCOUNT_ID) return null;
					throw error;
				});
				if (existing) {
					const amberiteUserId = existing.user.amberiteUserId ?? accountId;
					await ctx.runMutation(internal.auth.ensureLinkedMinecraftAccount, {
						userId: existing.user._id,
						amberiteUserId,
						accountId,
						gamertag,
						minecraftUuid,
					});
					return { userId: existing.user._id };
				}

				const accountProfile = personaProfile(gamertag, accountId, personaId);
				const { user } = await createAccount(ctx, {
					provider: MINECRAFT_TOKEN_PROVIDER_ID,
					account: { id: accountId },
					profile: accountProfile,
				});
				await ctx.runMutation(internal.auth.ensureLinkedMinecraftAccount, {
					userId: user._id,
					amberiteUserId: accountProfile.amberiteUserId,
					accountId,
					gamertag,
					minecraftUuid,
				});

				return { userId: user._id };
			},
		}),
	],
});

export const ensureLinkedMinecraftAccount = internalMutation({
	args: {
		userId: v.id("users"),
		amberiteUserId: v.string(),
		accountId: v.string(),
		gamertag: v.string(),
		minecraftUuid: v.string(),
	},
	handler: async (ctx, args) => {
		const user = await ctx.db.get(args.userId);
		if (!user) throw new Error("user does not exist");
		if (!user.amberiteUserId) {
			await ctx.db.patch(args.userId, { amberiteUserId: args.amberiteUserId });
		}

		const existing = await ctx.db
			.query("linkedMicrosoftAccounts")
			.withIndex("by_amberite_user", (q) => q.eq("amberiteUserId", args.amberiteUserId))
			.first();
		if (existing) {
			await ctx.db.patch(existing._id, {
				microsoftAccountId: args.accountId,
				gamertag: args.gamertag,
				minecraftUuid: args.minecraftUuid,
			});
			return;
		}
		await ctx.db.insert("linkedMicrosoftAccounts", {
			amberiteUserId: args.amberiteUserId,
			microsoftAccountId: args.accountId,
			gamertag: args.gamertag,
			minecraftUuid: args.minecraftUuid,
			linkedAt: Date.now(),
		});
	},
});

export const currentUser = query({
	args: {},
	handler: async (ctx) => {
		const userId = await getAuthUserId(ctx);
		if (userId === null) return null;
		const user = await ctx.db.get(userId);
		return user ? publicUser({ ...user, _id: userId }) : null;
	},
});

export const setUsername = mutation({
	args: {
		username: v.string(),
		displayName: v.optional(v.string()),
	},
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);

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
