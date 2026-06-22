import { httpAction } from "./_generated/server";
import { internal } from "./_generated/api";

const MAX_CLOCK_SKEW_MS = 30_000;

export const handle = httpAction(async (ctx, request) => {
	if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });
	const timestamp = request.headers.get("x-amberite-timestamp");
	const signature = request.headers.get("x-amberite-signature");
	if (!timestamp || !signature || Math.abs(Date.now() - Number(timestamp)) > MAX_CLOCK_SKEW_MS) return new Response("Unauthorized", { status: 401 });
	const body = await request.text();
	if (!(await validSignature(timestamp, body, signature))) return new Response("Unauthorized", { status: 401 });
	const input = JSON.parse(body) as { operation?: string; userId?: string; coreId?: string; credentialHash?: string; kind?: "desktop" | "core"; id?: string };
	if (input.operation === "desktopScope" && input.userId) return Response.json(await ctx.runQuery(internal.bridge.desktopScope, { userId: input.userId as never }));
	if (input.operation === "coreScope" && input.coreId && input.credentialHash) return Response.json(await ctx.runQuery(internal.bridge.coreScope, { coreId: input.coreId, credentialHash: input.credentialHash }));
	if (input.operation === "recipients" && input.kind && input.id) return Response.json(await ctx.runQuery(internal.bridge.recipients, { kind: input.kind, id: input.id, credentialHash: input.credentialHash }));
	return new Response("Bad request", { status: 400 });
});

async function validSignature(timestamp: string, body: string, signature: string): Promise<boolean> {
	const secret = process.env.REALTIME_BRIDGE_HMAC_SECRET;
	if (!secret) throw new Error("REALTIME_BRIDGE_HMAC_SECRET must be configured");
	const key = await crypto.subtle.importKey("raw", new TextEncoder().encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
	const expected = new Uint8Array(await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(`${timestamp}.${body}`)));
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
