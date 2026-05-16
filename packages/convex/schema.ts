import { authTables } from "@convex-dev/auth/server";
import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

const ackPolicy = v.union(v.literal("none"), v.literal("received"), v.literal("processed"));
const friendGroupRole = v.union(v.literal("owner"), v.literal("admin"), v.literal("member"));
const messageStatus = v.union(
	v.literal("pending"),
	v.literal("received"),
	v.literal("processed"),
	v.literal("expired"),
);
const pairingStatus = v.union(v.literal("waiting"), v.literal("claimed"), v.literal("expired"));

export default defineSchema({
	...authTables,
	users: defineTable({
		name: v.optional(v.string()),
		displayName: v.optional(v.string()),
		username: v.optional(v.string()),
		normalizedUsername: v.optional(v.string()),
		amberiteUserId: v.optional(v.string()),
		email: v.optional(v.string()),
		emailVerificationTime: v.optional(v.number()),
		phone: v.optional(v.string()),
		phoneVerificationTime: v.optional(v.number()),
		image: v.optional(v.string()),
		isAnonymous: v.optional(v.boolean()),
		onboardedAt: v.optional(v.number()),
	})
		.index("email", ["email"])
		.index("by_amberite_user_id", ["amberiteUserId"])
		.index("by_normalized_username", ["normalizedUsername"]),
	linkedMicrosoftAccounts: defineTable({
		amberiteUserId: v.string(),
		microsoftAccountId: v.string(),
		gamertag: v.optional(v.string()),
		minecraftUuid: v.optional(v.string()),
		metadata: v.optional(v.any()),
		linkedAt: v.number(),
	})
		.index("by_amberite_user", ["amberiteUserId"])
		.index("by_microsoft_account", ["microsoftAccountId"]),
	friendGroups: defineTable({
		name: v.optional(v.string()),
		ownerUserId: v.string(),
		coreId: v.optional(v.string()),
		createdAt: v.number(),
	})
		.index("by_owner", ["ownerUserId"])
		.index("by_core_id", ["coreId"]),
	friendGroupMembers: defineTable({
		friendGroupId: v.string(),
		userId: v.string(),
		role: friendGroupRole,
		createdAt: v.number(),
	})
		.index("by_group_user", ["friendGroupId", "userId"])
		.index("by_user", ["userId"]),
	pairingCores: defineTable({
		code: v.string(),
		coreId: v.string(),
		connectionUrl: v.optional(v.string()),
		status: pairingStatus,
		ownerUserId: v.optional(v.string()),
		metadata: v.optional(v.any()),
		createdAt: v.number(),
		expiresAt: v.number(),
		claimedAt: v.optional(v.number()),
	})
		.index("by_code", ["code"])
		.index("by_core_id", ["coreId"])
		.index("by_status", ["status"]),
	cores: defineTable({
		coreId: v.string(),
		ownerUserId: v.string(),
		friendGroupId: v.optional(v.string()),
		connectionUrl: v.optional(v.string()),
		lastSeenAt: v.number(),
		status: v.optional(v.string()),
		metadata: v.optional(v.any()),
	})
		.index("by_core_id", ["coreId"])
		.index("by_owner", ["ownerUserId"]),
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
});
