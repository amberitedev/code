import { requireUser } from '../common/auth'
import { randomId } from '../common/crypto'
import { ApiError, json } from '../common/http'
import type { Env } from '../common/types'

// Tickets. The App asks for a ticket for one Core and hands it to that Core, which verifies it with
// the public keys in `public/core/keys.json` and makes its own session. The account session never
// leaves the App. Signing stores nothing: one request, a few reads, zero writes.
//
// The private key is the `TICKET_KEY` secret, an Ed25519 JWK with its `kid`. README.md says how to
// make one and how to rotate it.

/** Every Core compares a ticket's `iss` to this. */
export const TICKET_ISSUER = 'amberite'
const TICKET_TTL_S = 5 * 60

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

/**
 * App: a ticket for one Core, for its owner or anyone in its access directory. Anyone else gets the
 * same 404 as for a Core that does not exist.
 */
export async function issueTicket(request: Request, env: Env, coreId: string) {
	const user = await requireUser(request, env)
	const allowed = await env.DB.prepare(
		'SELECT 1 FROM cores c WHERE c.id = ?1 AND (c.owner_id = ?2 OR EXISTS (SELECT 1 FROM core_access a WHERE a.core_id = c.id AND a.user_id = ?2))',
	)
		.bind(coreId, user.id)
		.first()
	if (!allowed) throw new ApiError(404, 'not_found', 'Core not found')

	const { kid, key } = await signingKey(env)
	const iat = Math.floor(Date.now() / 1000)
	const claims = {
		iss: TICKET_ISSUER,
		sub: user.id,
		name: user.username,
		avatar_url: user.avatar_url,
		aud: coreId,
		iat,
		exp: iat + TICKET_TTL_S,
		jti: randomId(32),
	}
	const signed = `${encodeJson({ alg: 'EdDSA', typ: 'JWT', kid })}.${encodeJson(claims)}`
	const signature = await crypto.subtle.sign('Ed25519', key, new TextEncoder().encode(signed))
	return json({ ticket: `${signed}.${base64url(new Uint8Array(signature))}` })
}
