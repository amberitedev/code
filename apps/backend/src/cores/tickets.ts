import {
	type FetchTicket,
	type PullTicket,
	type SessionTicket,
	type StoreHost,
	type StoreTicket,
	TICKET_ISSUER,
} from '@amberite/contracts/protocol'
import type { CoreTicket } from '@amberite/contracts/backend'

import { requireUser } from '../common/auth'
import { randomId } from '../common/crypto'
import { ApiError } from '../common/http'
import type { Env } from '../common/types'

// Tickets (`contracts::tickets`): short Ed25519 JWTs a Core verifies with the public keys in
// `public/core/keys.json`. The private key is the `TICKET_KEY` secret, an Ed25519 JWK with its
// `kid`; README.md says how to make one and how to rotate it.

const SESSION_TTL_S = 5 * 60
const STORE_TTL_S = 60 * 60
export const FETCH_TTL_S = 6 * 60 * 60
const PULL_TTL_S = 24 * 60 * 60

interface SigningKey {
	kid: string
	key: CryptoKey
}

// Imported once per isolate. Keyed by the secret so a rotated secret is picked up.
let cached: { secret: string; key: Promise<SigningKey> } | undefined

function signingKey(env: Env): Promise<SigningKey> {
	const secret = env.TICKET_KEY
	if (!secret)
		throw new ApiError(503, 'tickets_unavailable', 'This backend has no ticket key configured')
	if (cached?.secret !== secret) cached = { secret, key: importKey(secret) }
	return cached.key
}

async function importKey(secret: string): Promise<SigningKey> {
	const { kid, kty, crv, x, d }: Record<string, unknown> = JSON.parse(secret)
	if (
		typeof kid !== 'string' ||
		typeof x !== 'string' ||
		typeof d !== 'string' ||
		kty !== 'OKP' ||
		crv !== 'Ed25519'
	)
		throw new Error('TICKET_KEY must be an Ed25519 JWK with a kid')
	const key = await crypto.subtle.importKey('jwk', { kty, crv, x, d }, 'Ed25519', false, ['sign'])
	return { kid, key }
}

const base64url = (bytes: Uint8Array) =>
	btoa(String.fromCharCode(...bytes))
		.replace(/\+/g, '-')
		.replace(/\//g, '_')
		.replace(/=+$/, '')
const encodeJson = (value: unknown) => base64url(new TextEncoder().encode(JSON.stringify(value)))

type Ticket = SessionTicket | StoreTicket | FetchTicket | PullTicket

/** `claims` signed, good for `ttl` seconds from now. */
async function sign<T extends Ticket>(
	env: Env,
	claims: Omit<T, 'iss' | 'iat' | 'exp' | 'jti'>,
	ttl: number,
): Promise<{ ticket: string; expires: string }> {
	const { kid, key } = await signingKey(env)
	const iat = Math.floor(Date.now() / 1000)
	const body = { iss: TICKET_ISSUER, ...claims, iat, exp: iat + ttl, jti: randomId(32) }
	const signed = `${encodeJson({ alg: 'EdDSA', typ: 'JWT', kid })}.${encodeJson(body)}`
	const signature = await crypto.subtle.sign('Ed25519', key, new TextEncoder().encode(signed))
	return {
		ticket: `${signed}.${base64url(new Uint8Array(signature))}`,
		expires: new Date(body.exp * 1000).toISOString(),
	}
}

export async function storeTicket(env: Env, coreId: string, root: string, bytes: number) {
	return sign<StoreTicket>(env, { kind: 'store', aud: coreId, root, bytes }, STORE_TTL_S)
}

export async function fetchTicket(env: Env, coreIds: string[], root: string) {
	return (await sign<FetchTicket>(env, { kind: 'fetch', aud: coreIds, root }, FETCH_TTL_S)).ticket
}

/** A ticket for `coreId` to copy `root` from `source`, which holds it. */
export async function pullTicket(env: Env, coreId: string, root: string, source: StoreHost) {
	const ticket = await fetchTicket(env, [source.id], root)
	return (
		await sign<PullTicket>(
			env,
			{ kind: 'pull', aud: coreId, root, source: { core: source, ticket } },
			PULL_TTL_S,
		)
	).ticket
}

/**
 * App: a session ticket for one Core, for its owner. Anyone else gets the same 404 as for a Core
 * that does not exist.
 */
export async function issueTicket(request: Request, env: Env, coreId: string): Promise<CoreTicket> {
	const user = await requireUser(request, env)
	const allowed = await env.DB.prepare('SELECT 1 FROM cores WHERE id = ? AND owner_id = ?')
		.bind(coreId, user.id)
		.first()
	if (!allowed) throw new ApiError(404, 'not_found', 'Core not found')
	const { ticket } = await sign<SessionTicket>(
		env,
		{
			kind: 'session',
			sub: user.id,
			name: user.username,
			avatar_url: user.avatar_url,
			aud: coreId,
		},
		SESSION_TTL_S,
	)
	return { ticket }
}
