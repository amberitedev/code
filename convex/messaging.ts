import { getAuthUserId } from "@convex-dev/auth/server";
import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import type { MutationCtx, QueryCtx } from "./_generated/server";
import type { Doc } from "./_generated/dataModel";

const ackPolicy = v.union(v.literal("none"), v.literal("received"), v.literal("processed"));
const status = v.union(
	v.literal("pending"),
	v.literal("received"),
	v.literal("processed"),
	v.literal("expired"),
);

export const publishMessage = mutation({
	args: {
		messageId: v.string(),
		type: v.string(),
		version: v.number(),
		senderId: v.string(),
		recipientId: v.string(),
		payload: v.any(),
		ack: ackPolicy,
		ttlMs: v.number(),
	},
	returns: v.object({ messageId: v.string() }),
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);
		if (!(await canAccessEndpoint(ctx, userId, args.senderId)) || !(await canAccessEndpoint(ctx, userId, args.recipientId))) {
			throw new Error("not authorized for message endpoint");
		}

		const now = Date.now();
		const existing = await ctx.db
			.query("messages")
			.withIndex("by_message_id", (q) => q.eq("messageId", args.messageId))
			.unique();

		if (!existing) {
			await ctx.db.insert("messages", {
				messageId: args.messageId,
				type: args.type,
				version: args.version,
				senderId: args.senderId,
				recipientId: args.recipientId,
				payload: args.payload,
				ack: args.ack,
				status: "pending",
				createdAt: now,
				expiresAt: now + args.ttlMs,
				updatedAt: now,
			});
		}

		return { messageId: args.messageId };
	},
});

export const pendingMessages = query({
	args: { recipientId: v.string(), limit: v.optional(v.number()) },
	returns: v.array(v.any()),
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);
		await requireEndpointAccess(ctx, userId, args.recipientId);

		const now = Date.now();
		const messages = await ctx.db
			.query("messages")
			.withIndex("by_recipient_status", (q) =>
				q.eq("recipientId", args.recipientId).eq("status", "pending"),
			)
			.take(args.limit ?? 50);

		return messages.filter((message) => message.expiresAt > now);
	},
});

export const ackMessage = mutation({
	args: { messageId: v.string(), recipientId: v.string() },
	returns: v.null(),
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);
		await requireEndpointAccess(ctx, userId, args.recipientId);

		const message = await messageById(ctx, args.messageId);
		if (!message || message.recipientId !== args.recipientId) return null;
		const now = Date.now();

		await ctx.db.patch(message._id, {
			status: message.ack === "received" ? "processed" : "received",
			updatedAt: now,
		});
		await ctx.db.insert("receipts", {
			messageId: args.messageId,
			recipientId: args.recipientId,
			receivedAt: now,
		});
		return null;
	},
});

export const completeMessage = mutation({
	args: {
		messageId: v.string(),
		recipientId: v.string(),
		result: v.optional(v.any()),
		error: v.optional(v.string()),
	},
	returns: v.null(),
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);
		await requireEndpointAccess(ctx, userId, args.recipientId);

		const message = await messageById(ctx, args.messageId);
		if (!message || message.recipientId !== args.recipientId) return null;
		const now = Date.now();

		await ctx.db.patch(message._id, { status: "processed", updatedAt: now });
		await ctx.db.insert("receipts", {
			messageId: args.messageId,
			recipientId: args.recipientId,
			processedAt: now,
			result: args.result,
			error: args.error,
		});
		return null;
	},
});

export const messageStatus = query({
	args: { messageId: v.string() },
	returns: v.union(v.null(), v.object({ status, result: v.optional(v.any()), error: v.optional(v.string()) })),
	handler: async (ctx, args) => {
		const userId = await requireUserId(ctx);
		const message = await messageById(ctx, args.messageId);
		if (!message) return null;
		if (!(await canAccessEndpoint(ctx, userId, message.senderId)) && !(await canAccessEndpoint(ctx, userId, message.recipientId))) {
			throw new Error("not authorized for message");
		}

		const receipt = await ctx.db
			.query("receipts")
			.withIndex("by_message_id", (q) => q.eq("messageId", args.messageId))
			.order("desc")
			.first();
		return { status: message.status, result: receipt?.result, error: receipt?.error };
	},
});

async function messageById(ctx: QueryCtx | MutationCtx, messageId: string): Promise<Doc<"messages"> | null> {
	return await ctx.db
		.query("messages")
		.withIndex("by_message_id", (q) => q.eq("messageId", messageId))
		.unique();
}

async function requireUserId(ctx: QueryCtx | MutationCtx): Promise<string> {
	const userId = await getAuthUserId(ctx);
	if (userId === null) throw new Error("not authenticated");
	return userId;
}

async function requireEndpointAccess(ctx: QueryCtx | MutationCtx, userId: string, endpointId: string): Promise<void> {
	if (!(await canAccessEndpoint(ctx, userId, endpointId))) throw new Error("not authorized for endpoint");
}

async function canAccessEndpoint(ctx: QueryCtx | MutationCtx, userId: string, endpointId: string): Promise<boolean> {
	if (endpointId === userId) return true;
	const core = await ctx.db
		.query("cores")
		.withIndex("by_core_id", (q) => q.eq("coreId", endpointId))
		.unique();
	if (!core) return false;
	if (core.ownerUserId === userId) return true;
	const friendGroupId = core.friendGroupId;
	if (!friendGroupId) return false;
	const membership = await ctx.db
		.query("friendGroupMembers")
		.withIndex("by_group_user", (q) => q.eq("friendGroupId", friendGroupId).eq("userId", userId))
		.unique();
	return Boolean(membership);
}
