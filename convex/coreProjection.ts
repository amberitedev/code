import { v } from 'convex/values'
import { httpAction, internalMutation } from './_generated/server'
import { internal } from './_generated/api'

const MAX_BODY_BYTES = 64 * 1024
const MAX_MEMBERS = 1_000

const linkState = v.union(v.literal('unlinked'), v.literal('linked'))
const setupMode = v.union(v.literal('remote'), v.literal('local'))

export const syncSnapshot = httpAction(async (ctx, request) => {
	if (request.method !== 'POST') return new Response('Method not allowed', { status: 405 })
	const credential = bearer(request) ?? request.headers.get('x-amberite-core-credential')
	if (!credential || !/^[a-f0-9]{64}$/i.test(credential))
		return new Response('Unauthorized', { status: 401 })
	const body = await readBoundedBody(request)
	if (body === null) return new Response('Payload too large', { status: 413 })
	const snapshot = parseSnapshot(body)
	if (!snapshot) return new Response('Bad request', { status: 400 })
	const result = await ctx.runMutation(internal.coreProjection.applySnapshot, {
		credentialHash: await hashCredential(credential),
		snapshot,
	})
	if (result.status === 'unauthorized') return new Response('Unauthorized', { status: 401 })
	return Response.json(result)
})

export const applySnapshot = internalMutation({
	args: {
		credentialHash: v.string(),
		snapshot: v.object({
			coreId: v.string(),
			ownerUserId: v.string(),
			linkState,
			connectionUrl: v.optional(v.string()),
			setupMode: v.optional(setupMode),
			members: v.array(v.object({ userId: v.string(), isOwner: v.optional(v.boolean()) })),
			revision: v.number(),
			lastSeenAt: v.optional(v.number()),
			syncedAt: v.optional(v.number()),
		}),
	},
	handler: async (ctx, args) => {
		const { snapshot } = args
		const [coreList, legacyCore] = await Promise.all([
			ctx.db
				.query('coreList')
				.withIndex('by_core_id', (q) => q.eq('coreId', snapshot.coreId))
				.unique(),
			ctx.db
				.query('cores')
				.withIndex('by_core_id', (q) => q.eq('coreId', snapshot.coreId))
				.unique(),
		])
		const expectedHash = coreList?.syncCredentialHash ?? legacyCore?.realtimeCredentialHash
		if (!expectedHash || !constantTimeEquals(expectedHash, args.credentialHash))
			return { ok: false, status: 'unauthorized' as const }
		const expectedOwner = coreList?.ownerUserId ?? legacyCore?.ownerUserId
		if (expectedOwner && expectedOwner !== snapshot.ownerUserId)
			return { ok: false, status: 'unauthorized' as const }
		if (coreList && coreList.projectionRevision > snapshot.revision) {
			return {
				ok: true,
				status: 'stale' as const,
				coreId: snapshot.coreId,
				projectionRevision: coreList.projectionRevision,
				syncedAt: coreList.syncedAt,
			}
		}

		const now = Date.now()
		const syncedAt = snapshot.syncedAt ?? now
		const coreValue = {
			coreId: snapshot.coreId,
			ownerUserId: snapshot.ownerUserId,
			linkState: snapshot.linkState,
			connectionUrl: snapshot.connectionUrl,
			setupMode: snapshot.setupMode,
			lastSeenAt: snapshot.lastSeenAt ?? now,
			projectionRevision: snapshot.revision,
			syncedAt,
			syncCredentialHash: expectedHash,
		}
		if (coreList) await ctx.db.patch(coreList._id, coreValue)
		else await ctx.db.insert('coreList', { ...coreValue, createdAt: now })

		const members = new Map<string, boolean>()
		members.set(snapshot.ownerUserId, true)
		for (const member of snapshot.members) {
			members.set(member.userId, member.isOwner === true || member.userId === snapshot.ownerUserId)
		}
		const existingLinks = await ctx.db
			.query('coreMemberLinks')
			.withIndex('by_core', (q) => q.eq('coreId', snapshot.coreId))
			.collect()
		for (const link of existingLinks) {
			if (!members.has(link.userId)) await ctx.db.delete(link._id)
		}
		for (const [userId, isOwner] of members) {
			const existing = existingLinks.find((link) => link.userId === userId)
			const value = { coreId: snapshot.coreId, userId, isOwner, syncedAt }
			if (existing) await ctx.db.patch(existing._id, value)
			else await ctx.db.insert('coreMemberLinks', value)
		}

		return {
			ok: true,
			status: 'applied' as const,
			coreId: snapshot.coreId,
			projectionRevision: snapshot.revision,
			syncedAt,
		}
	},
})

type ProjectionSnapshot = {
	coreId: string
	ownerUserId: string
	linkState: 'unlinked' | 'linked'
	connectionUrl?: string
	setupMode?: 'remote' | 'local'
	members: Array<{ userId: string; isOwner?: boolean }>
	revision: number
	lastSeenAt?: number
	syncedAt?: number
}

function parseSnapshot(body: string): ProjectionSnapshot | null {
	let value: unknown
	try {
		value = JSON.parse(body)
	} catch {
		return null
	}
	if (!isRecord(value)) return null
	if (
		!validId(value.coreId) ||
		!validId(value.ownerUserId) ||
		(value.linkState !== 'linked' && value.linkState !== 'unlinked') ||
		!validRevision(value.revision) ||
		!Array.isArray(value.members) ||
		value.members.length > MAX_MEMBERS
	) {
		return null
	}
	if (value.connectionUrl !== undefined && !validOptionalString(value.connectionUrl, 2_048))
		return null
	if (value.setupMode !== undefined && value.setupMode !== 'remote' && value.setupMode !== 'local')
		return null
	if (value.lastSeenAt !== undefined && !validTimestamp(value.lastSeenAt)) return null
	if (value.syncedAt !== undefined && !validTimestamp(value.syncedAt)) return null
	const members: Array<{ userId: string; isOwner?: boolean }> = []
	for (const member of value.members) {
		if (!isRecord(member) || !validId(member.userId)) return null
		if (member.isOwner !== undefined && typeof member.isOwner !== 'boolean') return null
		members.push({ userId: member.userId, ...(member.isOwner !== undefined ? { isOwner: member.isOwner } : {}) })
	}
	return {
		coreId: value.coreId,
		ownerUserId: value.ownerUserId,
		linkState: value.linkState,
		...(value.connectionUrl !== undefined ? { connectionUrl: value.connectionUrl } : {}),
		...(value.setupMode !== undefined ? { setupMode: value.setupMode } : {}),
		members,
		revision: value.revision,
		...(value.lastSeenAt !== undefined ? { lastSeenAt: value.lastSeenAt } : {}),
		...(value.syncedAt !== undefined ? { syncedAt: value.syncedAt } : {}),
	}
}

async function hashCredential(credential: string): Promise<string> {
	const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(credential)))
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
	for (let index = 0; index < leftBytes.length; index++) difference |= leftBytes[index] ^ rightBytes[index]
	return difference === 0
}

function bearer(request: Request): string | null {
	const value = request.headers.get('authorization')
	return value?.startsWith('Bearer ') ? value.slice(7) : null
}

async function readBoundedBody(request: Request): Promise<string | null> {
	const contentLength = request.headers.get('content-length')
	if (contentLength && (!/^\d+$/.test(contentLength) || Number(contentLength) > MAX_BODY_BYTES)) return null
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

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function validId(value: unknown): value is string {
	return typeof value === 'string' && value.length > 0 && value.length <= 256
}

function validOptionalString(value: unknown, max: number): value is string {
	return typeof value === 'string' && value.length <= max
}

function validRevision(value: unknown): value is number {
	return typeof value === 'number' && Number.isInteger(value) && value >= 0
}

function validTimestamp(value: unknown): value is number {
	return typeof value === 'number' && Number.isFinite(value) && value >= 0
}
