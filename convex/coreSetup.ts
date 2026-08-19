import { v } from 'convex/values'
import { httpAction, internalMutation } from './_generated/server'
import { internal } from './_generated/api'

const MAX_BODY_BYTES = 1024

export const verifyClaimHttp = httpAction(async (ctx, request) => {
	if (request.method !== 'POST') return new Response('Method not allowed', { status: 405 })
	const credential = bearer(request)
	if (!credential || !/^[a-f0-9]{64}$/i.test(credential))
		return new Response('Unauthorized', { status: 401 })
	const body = await readBoundedBody(request)
	if (body === null) return new Response('Payload too large', { status: 413 })
	const input = parseRequest(body)
	if (!input) return new Response('Bad request', { status: 400 })
	const ok = await ctx.runMutation(internal.coreSetup.verifyClaim, {
		...input,
		credentialHash: await hashCredential(credential),
		now: Date.now(),
	})
	if (!ok) return new Response('Unauthorized', { status: 401 })
	return Response.json({ ok: true })
})

export const verifyClaim = internalMutation({
	args: {
		coreId: v.string(),
		ownerUserId: v.string(),
		credentialHash: v.string(),
		now: v.number(),
	},
	returns: v.boolean(),
	handler: async (ctx, args) => {
		const ownerUserId = ctx.db.normalizeId('users', args.ownerUserId)
		if (!ownerUserId) return false
		const pairing = await ctx.db
			.query('pairingCores')
			.withIndex('by_core_id', (q) => q.eq('coreId', args.coreId))
			.unique()
		if (
			!pairing ||
			pairing.status !== 'claimed' ||
			pairing.ownerUserId !== ownerUserId ||
			!pairing.syncCredentialHash ||
			pairing.expiresAt <= args.now
		) {
			return false
		}
		return constantTimeEquals(pairing.syncCredentialHash, args.credentialHash)
	},
})

type SetupClaimRequest = {
	coreId: string
	ownerUserId: string
}

function parseRequest(body: string): SetupClaimRequest | null {
	let value: unknown
	try {
		value = JSON.parse(body)
	} catch {
		return null
	}
	if (!isRecord(value) || !validId(value.coreId) || !validId(value.ownerUserId)) return null
	return { coreId: value.coreId, ownerUserId: value.ownerUserId }
}

function bearer(request: Request): string | null {
	const value = request.headers.get('authorization')
	return value?.startsWith('Bearer ') ? value.slice(7) : null
}

async function readBoundedBody(request: Request): Promise<string | null> {
	const contentLength = request.headers.get('content-length')
	if (contentLength && (!/^\d+$/.test(contentLength) || Number(contentLength) > MAX_BODY_BYTES))
		return null
	const reader = request.body?.getReader()
	if (!reader) return ''
	const chunks: Uint8Array[] = []
	let length = 0
	try {
		while (true) {
			const { done, value } = await reader.read()
			if (done) break
			length += value.byteLength
			if (length > MAX_BODY_BYTES) return null
			chunks.push(value)
		}
	} finally {
		reader.releaseLock()
	}
	const bytes = new Uint8Array(length)
	let offset = 0
	for (const chunk of chunks) {
		bytes.set(chunk, offset)
		offset += chunk.byteLength
	}
	return new TextDecoder().decode(bytes)
}

async function hashCredential(credential: string): Promise<string> {
	const digest = new Uint8Array(
		await crypto.subtle.digest('SHA-256', new TextEncoder().encode(credential)),
	)
	return base64Url(digest)
}

function base64Url(bytes: Uint8Array): string {
	let value = ''
	for (const byte of bytes) value += String.fromCharCode(byte)
	return btoa(value).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '')
}

function constantTimeEquals(left: string, right: string): boolean {
	const leftBytes = new TextEncoder().encode(left)
	const rightBytes = new TextEncoder().encode(right)
	if (leftBytes.length !== rightBytes.length) return false
	let difference = 0
	for (let index = 0; index < leftBytes.length; index++)
		difference |= leftBytes[index] ^ rightBytes[index]
	return difference === 0
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function validId(value: unknown): value is string {
	return typeof value === 'string' && value.length > 0 && value.length <= 256
}
