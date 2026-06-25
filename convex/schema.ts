import { authTables } from "@convex-dev/auth/server";
import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

const ackPolicy = v.union(v.literal("none"), v.literal("received"), v.literal("processed"));
const friendGroupRole = v.union(v.literal("owner"), v.literal("admin"), v.literal("member"));
const friendRequestStatus = v.union(v.literal("pending"), v.literal("accepted"), v.literal("declined"), v.literal("canceled"));
const groupInviteStatus = v.union(v.literal("pending"), v.literal("accepted"), v.literal("declined"), v.literal("revoked"), v.literal("expired"));
const coreLinkState = v.union(v.literal("unlinked"), v.literal("linked"));
const linkedAccountStatus = v.union(v.literal("active"), v.literal("needs_reconnect"), v.literal("revoked"));
const messageStatus = v.union(v.literal("pending"), v.literal("received"), v.literal("processed"), v.literal("expired"));
const pairingStatus = v.union(v.literal("waiting"), v.literal("claimed"), v.literal("expired"));
const syncStatus = v.union(v.literal("active"), v.literal("paused"), v.literal("archived"));
const profileVisibility = v.union(v.literal("everyone"), v.literal("roles"), v.literal("custom"));
const whitelistScope = v.union(v.literal("viewers"), v.literal("roles"), v.literal("custom"));

export default defineSchema({
	...authTables,
	users: defineTable({
		name: v.optional(v.string()), displayName: v.optional(v.string()), username: v.optional(v.string()),
		normalizedUsername: v.optional(v.string()), friendCode: v.optional(v.string()),
		amberiteUserId: v.optional(v.string()), email: v.optional(v.string()), emailVerificationTime: v.optional(v.number()),
		phone: v.optional(v.string()), phoneVerificationTime: v.optional(v.number()), image: v.optional(v.string()),
		bio: v.optional(v.string()), avatarUrl: v.optional(v.string()), avatarStorageId: v.optional(v.string()),
		avatarMimeType: v.optional(v.string()), avatarSizeBytes: v.optional(v.number()),
		profileUpdatedAt: v.optional(v.number()), deletedAt: v.optional(v.number()),
		deletedReason: v.optional(v.string()), isAnonymous: v.optional(v.boolean()),
		onboardedAt: v.optional(v.number()),
	})
		.index("email", ["email"])
		.index("by_amberite_user_id", ["amberiteUserId"])
		.index("by_friend_code", ["friendCode"])
		.index("by_normalized_username", ["normalizedUsername"]),
	linkedMicrosoftAccounts: defineTable({
		amberiteUserId: v.string(), microsoftAccountId: v.string(), gamertag: v.optional(v.string()),
		minecraftUuid: v.optional(v.string()), metadata: v.optional(v.any()), linkedAt: v.number(),
	})
		.index("by_amberite_user", ["amberiteUserId"])
		.index("by_microsoft_account", ["microsoftAccountId"]),
	linkedModrinthAccounts: defineTable({
		userId: v.id("users"), modrinthUserId: v.string(), username: v.string(),
		avatarUrl: v.optional(v.string()), scopes: v.array(v.string()),
		encryptedAccessToken: v.string(), encryptedRefreshToken: v.optional(v.string()),
		expiresAt: v.optional(v.number()), status: linkedAccountStatus,
		needsReconnect: v.optional(v.boolean()), reconnectReason: v.optional(v.string()),
		linkedAt: v.number(), updatedAt: v.number(),
	})
		.index("by_user", ["userId"])
		.index("by_modrinth_user", ["modrinthUserId"]),
	/** Deprecated migration authority. Core owns names, roles, permissions, bans, invites, and audit data. */
	friendGroups: defineTable({
		name: v.optional(v.string()), description: v.optional(v.string()), banner: v.optional(v.string()),
		subdomain: v.optional(v.string()), ownerUserId: v.string(), coreId: v.optional(v.string()),
		createdAt: v.number(), updatedAt: v.optional(v.number()),
	})
		.index("by_owner", ["ownerUserId"])
		.index("by_core_id", ["coreId"]),
	friendGroupMembers: defineTable({
		friendGroupId: v.string(), userId: v.string(), role: friendGroupRole,
		permissionPreset: v.optional(v.string()), customPermissions: v.optional(v.any()),
		createdAt: v.number(), updatedAt: v.optional(v.number()),
	})
		.index("by_group_user", ["friendGroupId", "userId"])
		.index("by_group", ["friendGroupId"])
		.index("by_user_group", ["userId", "friendGroupId"])
		.index("by_user", ["userId"]),
	friendRequests: defineTable({
		fromUserId: v.string(), toUserId: v.string(), status: friendRequestStatus,
		message: v.optional(v.string()), createdAt: v.number(), updatedAt: v.number(),
		notificationClaimedAt: v.optional(v.number()), notificationClaimExpiresAt: v.optional(v.number()),
		notificationDeliveredAt: v.optional(v.number()),
	})
		.index("by_from_to", ["fromUserId", "toUserId"])
		.index("by_to_status", ["toUserId", "status"])
		.index("by_from_status", ["fromUserId", "status"]),
	friendships: defineTable({ userAId: v.string(), userBId: v.string(), createdAt: v.number() })
		.index("by_pair", ["userAId", "userBId"])
		.index("by_user_a", ["userAId"])
		.index("by_user_b", ["userBId"]),
	blockedUsers: defineTable({ blockerUserId: v.string(), blockedUserId: v.string(), createdAt: v.number() })
		.index("by_blocker_blocked", ["blockerUserId", "blockedUserId"])
		.index("by_blocker", ["blockerUserId"]),
	friendGroupBans: defineTable({
		friendGroupId: v.string(), userId: v.string(), bannedByUserId: v.string(),
		reason: v.optional(v.string()), createdAt: v.number(),
	})
		.index("by_group_user", ["friendGroupId", "userId"])
		.index("by_group", ["friendGroupId"]),
	friendGroupInvites: defineTable({
		friendGroupId: v.string(), inviterUserId: v.string(), inviteeUserId: v.optional(v.string()),
		code: v.optional(v.string()), role: friendGroupRole, status: groupInviteStatus,
		createdAt: v.number(), expiresAt: v.number(), respondedAt: v.optional(v.number()),
	})
		.index("by_code", ["code"])
		.index("by_invitee_status", ["inviteeUserId", "status"])
		.index("by_group_status", ["friendGroupId", "status"]),
	pairingCores: defineTable({
		code: v.string(), coreId: v.string(), connectionUrl: v.optional(v.string()), status: pairingStatus,
		ownerUserId: v.optional(v.string()), metadata: v.optional(v.any()), createdAt: v.number(),
		expiresAt: v.number(), claimedAt: v.optional(v.number()),
		realtimeCredentialHash: v.optional(v.string()), realtimeCredentialIssuedAt: v.optional(v.number()),
	})
		.index("by_code", ["code"])
		.index("by_core_id", ["coreId"])
		.index("by_expires_at", ["expiresAt"])
		.index("by_status", ["status"]),
	coreList: defineTable({
		coreId: v.string(), ownerUserId: v.string(), linkState: coreLinkState,
		connectionUrl: v.optional(v.string()), setupMode: v.optional(v.union(v.literal("remote"), v.literal("local"))),
		createdAt: v.number(), lastSeenAt: v.number(), projectionRevision: v.number(),
		syncedAt: v.number(), syncCredentialHash: v.optional(v.string()),
	})
		.index("by_core_id", ["coreId"])
		.index("by_owner", ["ownerUserId"])
		.index("by_link_state", ["linkState"]),
	coreMemberLinks: defineTable({
		coreId: v.string(), userId: v.string(), isOwner: v.boolean(), syncedAt: v.number(),
	})
		.index("by_core_user", ["coreId", "userId"])
		.index("by_core", ["coreId"])
		.index("by_user", ["userId"]),
	/** Deprecated migration authority. Use coreList and coreMemberLinks for new Convex/Core projection reads. */
	cores: defineTable({
		coreId: v.string(),
		ownerUserId: v.string(),
		friendGroupId: v.optional(v.string()),
		name: v.optional(v.string()),
		subdomain: v.optional(v.string()),
		setupMode: v.optional(v.union(v.literal("remote"), v.literal("local"))),
		connectionUrl: v.optional(v.string()),
		lastSeenAt: v.number(),
		status: v.optional(v.string()), realtimeCredentialHash: v.optional(v.string()),
		metadata: v.optional(v.any()),
	})
		.index("by_core_id", ["coreId"])
		.index("by_friend_group", ["friendGroupId"])
		.index("by_owner", ["ownerUserId"]),
	syncedProfiles: defineTable({
		friendGroupId: v.string(),
		coreId: v.string(),
		coreInstanceId: v.string(),
		clientProfileId: v.optional(v.string()),
		name: v.string(),
		gameVersion: v.optional(v.string()),
		loader: v.optional(v.string()),
		syncEnabled: v.boolean(),
		status: syncStatus,
		visibility: v.optional(profileVisibility),
		visibilityRoles: v.optional(v.array(friendGroupRole)),
		visibilityUserIds: v.optional(v.array(v.string())),
		autoWhitelist: v.optional(v.boolean()),
		whitelistScope: v.optional(whitelistScope),
		whitelistRoles: v.optional(v.array(friendGroupRole)),
		whitelistUserIds: v.optional(v.array(v.string())),
		createdAt: v.number(),
		updatedAt: v.number(),
	})
		.index("by_group", ["friendGroupId"])
		.index("by_core_instance", ["coreId", "coreInstanceId"]),
	profileSnapshots: defineTable({
		profileId: v.string(),
		authorUserId: v.string(),
		manifest: v.any(),
		clientOnlyManifest: v.optional(v.any()),
		serverManifest: v.optional(v.any()),
		notes: v.optional(v.string()),
		createdAt: v.number(),
	})
		.index("by_profile", ["profileId"])
		.index("by_author", ["authorUserId"]),
	modSyncEvents: defineTable({
		profileId: v.string(),
		snapshotId: v.string(),
		authorUserId: v.string(),
		status: v.union(v.literal("planned"), v.literal("applied"), v.literal("failed")),
		diff: v.optional(v.any()),
		message: v.optional(v.string()),
		createdAt: v.number(),
	})
		.index("by_profile", ["profileId"])
		.index("by_snapshot", ["snapshotId"]),
	messages: defineTable({
		messageId: v.string(),
		type: v.string(),
		version: v.number(),
		senderId: v.string(),
		recipientId: v.string(),
		payload: v.any(),
		ack: ackPolicy,
		status: messageStatus,
		createdAt: v.number(),
		expiresAt: v.number(),
		updatedAt: v.number(),
	})
		.index("by_message_id", ["messageId"])
		.index("by_recipient_status", ["recipientId", "status"])
		.index("by_sender", ["senderId"]),
	receipts: defineTable({
		messageId: v.string(),
		recipientId: v.string(),
		receivedAt: v.optional(v.number()),
		processedAt: v.optional(v.number()),
		result: v.optional(v.any()),
		error: v.optional(v.string()),
	})
		.index("by_message_id", ["messageId"])
		.index("by_recipient", ["recipientId"]),
	realtimeBridgeRequests: defineTable({
		requestId: v.string(), expiresAt: v.number(),
	})
		.index("by_request_id", ["requestId"])
		.index("by_expires_at", ["expiresAt"]),
});
