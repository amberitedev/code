import { v } from "convex/values";
import { httpAction, internalMutation } from "./_generated/server";
import { internal } from "./_generated/api";

const MAX_CLOCK_SKEW_MS = 30_000;
const MAX_BODY_BYTES = 2048;

export const consumeRequest = internalMutation({
	args: { requestId: v.string(), expiresAt: v.number() },
	returns: v.boolean(),
	handler: async (ctx, args) => {
		const existing = await ctx.db
			.query("realtimeBridgeRequests")
			.withIndex("by_request_id", (q) => q.eq("requestId", args.requestId))
			.unique();
		const now = Date.now();
		if (existing && existing.expiresAt > now) return false;
		if (existing) await ctx.db.patch(existing._id, { expiresAt: args.expiresAt });
		else await ctx.db.insert("realtimeBridgeRequests", args);
		return true;
	},
});

export const cleanupExpiredRequests = internalMutation({
	args: {},
	returns: v.number(),
	handler: async (ctx) => {
		const expired = await ctx.db
			.query("realtimeBridgeRequests")
			.withIndex("by_expires_at", (q) => q.lte("expiresAt", Date.now()))
			.take(100);
		await Promise.all(expired.map((request) => ctx.db.delete(request._id)));
		return expired.length;
	},
});

export const handle = httpAction(async (ctx, request) => {
	if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });
	const timestamp = request.headers.get("x-amberite-timestamp");
	const signature = request.headers.get("x-amberite-signature");
	const requestId = request.headers.get("x-amberite-request-id");
	if (!validTimestamp(timestamp) || !validRequestId(requestId) || !validSignatureValue(signature))
		return new Response("Unauthorized", { status: 401 });
	const body = await readBoundedBody(request);
	if (body === null || !(await validSignature(timestamp, requestId, body, signature)))
		return new Response("Unauthorized", { status: 401 });
	const accepted = await ctx.runMutation(internal.realtimeBridge.consumeRequest, {
		requestId,
		expiresAt: Number(timestamp) + MAX_CLOCK_SKEW_MS,
	});
	if (!accepted) return new Response("Replay rejected", { status: 409 });
	const input = parseRequest(body);
	if (!input) return new Response("Bad request", { status: 400 });
	if (input.operation === "desktopScope") return Response.json(await ctx.runQuery(internal.bridge.desktopScope, { userId: input.userId as never }));
	if (input.operation === "recipients") return Response.json(await ctx.runQuery(internal.bridge.recipients, input));
	return new Response("Bad request", { status: 400 });
});

async function validSignature(timestamp: string, requestId: string, body: string, signature: string): Promise<boolean> {
	const secret = process.env.REALTIME_BRIDGE_HMAC_SECRET;
	if (!secret) throw new Error("REALTIME_BRIDGE_HMAC_SECRET must be configured");
	const key = await crypto.subtle.importKey("raw", new TextEncoder().encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
	const expected = new Uint8Array(await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(`${timestamp}.${requestId}.${body}`)));
	const received = fromBase64Url(signature);
	if (expected.length !== received.length) return false;
	let difference = 0;
	for (let index = 0; index < expected.length; index++) difference |= expected[index] ^ received[index];
	return difference === 0;
}

function fromBase64Url(value: string): Uint8Array {
	const base64 = value.replace(/-/g, "+").replace(/_/g, "/") + "=".repeat((4 - value.length % 4) % 4);
	return Uint8Array.from(atob(base64), (character) => character.charCodeAt(0));
}

function validTimestamp(value: string | null): value is string {
	if (!value || !/^\d{13}$/.test(value)) return false;
	return Math.abs(Date.now() - Number(value)) <= MAX_CLOCK_SKEW_MS;
}

function validRequestId(value: string | null): value is string {
	return Boolean(value && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value));
}

function validSignatureValue(value: string | null): value is string {
	return Boolean(value && /^[A-Za-z0-9_-]{43}$/.test(value));
}

async function readBoundedBody(request: Request): Promise<string | null> {
	const contentLength = request.headers.get("content-length");
	if (contentLength && (!/^\d+$/.test(contentLength) || Number(contentLength) > MAX_BODY_BYTES)) return null;
	const reader = request.body?.getReader();
	if (!reader) return "";
	const chunks: Uint8Array[] = [];
	let length = 0;
	try {
		while (true) {
			const { done, value } = await reader.read();
			if (done) break;
			length += value.byteLength;
			if (length > MAX_BODY_BYTES) return null;
			chunks.push(value);
		}
	} finally {
		reader.releaseLock();
	}
	const bytes = new Uint8Array(length);
	let offset = 0;
	for (const chunk of chunks) {
		bytes.set(chunk, offset);
		offset += chunk.byteLength;
	}
	return new TextDecoder().decode(bytes);
}

type BridgeRequest =
	| { operation: "desktopScope"; userId: string }
	| { operation: "recipients"; kind: "desktop"; id: string };

function parseRequest(body: string): BridgeRequest | null {
	let value: unknown;
	try {
		value = JSON.parse(body);
	} catch {
		return null;
	}
	if (!isRecord(value) || typeof value.operation !== "string") return null;
	if (value.operation === "desktopScope" && exactKeys(value, ["operation", "userId"]) && validId(value.userId))
		return { operation: "desktopScope", userId: value.userId };
	if (value.operation === "recipients" && exactKeys(value, ["operation", "kind", "id"]) && value.kind === "desktop" && validId(value.id))
		return { operation: "recipients", kind: value.kind, id: value.id };
	return null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function exactKeys(value: Record<string, unknown>, allowed: string[]): boolean {
	const keys = Object.keys(value);
	return keys.every((key) => allowed.includes(key)) && keys.length === allowed.length;
}

function validId(value: unknown): value is string {
	return typeof value === "string" && value.length > 0 && value.length <= 256;
}
