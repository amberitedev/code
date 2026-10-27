import { requireUser } from '../common/auth'
import { randomId } from '../common/crypto'
import { ApiError, json } from '../common/http'
import type { Env } from '../common/types'

// Tickets: short Ed25519 JWTs a Core verifies with the public keys in `public/core/keys.json`.
// Every ticket carries a `kind`, and a Core takes each kind on its own routes only:
//
// - `session`: the App signs in to one Core as one user. Five minutes, one use.
// - `store`: the owner's App uploads one version (`root`) of at most `bytes` to the home Core.
// - `fetch`: a member's App reads one version from any of the Cores in `aud`.
// - `pull`: a Core copies one version from `source`, with a fetch ticket for it inside.
//
// Signing stores nothing. The private key is the `TICKET_KEY` secret, an Ed25519 JWK with its
// `kid`; README.md says how to make one and how to rotate it.

/** Every Core compares a ticket's `iss` to this. */
export const TICKET_ISSUER = 'amberite'
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

/** A signed ticket of `kind` with `claims`, good for `ttl` seconds from now. */
async function sign(
	env: Env,
	kind: string,
	claims: Record<string, unknown>,
	ttl: number,
): Promise<{ ticket: string; expires: string }> {
	const { kid, key } = await signingKey(env)
	const iat = Math.floor(Date.now() / 1000)
	const body = { iss: TICKET_ISSUER, kind, ...claims, iat, exp: iat + ttl, jti: randomId(32) }
	const signed = `${encodeJson({ alg: 'EdDSA', typ: 'JWT', kid })}.${encodeJson(body)}`
	const signature = await crypto.subtle.sign('Ed25519', key, new TextEncoder().encode(signed))
	return {
		ticket: `${signed}.${base64url(new Uint8Array(signature))}`,
		expires: new Date(body.exp * 1000).toISOString(),
	}
}

/** A Core as a ticket names it and the App reaches it. */
export interface Holder {
	id: string
	address: string
	public_address: string | null
	cert_fingerprint: string | null
}

export async function storeTicket(env: Env, coreId: string, root: string, bytes: number) {
	return sign(env, 'store', { aud: coreId, root, bytes }, STORE_TTL_S)
}

export async function fetchTicket(env: Env, coreIds: string[], root: string) {
	return (await sign(env, 'fetch', { aud: coreIds, root }, FETCH_TTL_S)).ticket
}

/** A ticket for `coreId` to copy `root` from `source`, which holds it. */
export async function pullTicket(env: Env, coreId: string, root: string, source: Holder) {
	const ticket = await fetchTicket(env, [source.id], root)
	return (
		await sign(env, 'pull', { aud: coreId, root, source: { core: source, ticket } }, PULL_TTL_S)
	).ticket
}

/**
 * App: a session ticket for one Core, for its owner or anyone in its access directory. Anyone
 * else gets the same 404 as for a Core that does not exist.
 */
export async function issueTicket(request: Request, env: Env, coreId: string) {
	const user = await requireUser(request, env)
	const allowed = await env.DB.prepare(
		'SELECT 1 FROM cores c WHERE c.id = ?1 AND (c.owner_id = ?2 OR EXISTS (SELECT 1 FROM core_access a WHERE a.core_id = c.id AND a.user_id = ?2))',
	)
		.bind(coreId, user.id)
		.first()
	if (!allowed) throw new ApiError(404, 'not_found', 'Core not found')
	const { ticket } = await sign(
		env,
		'session',
		{ sub: user.id, name: user.username, avatar_url: user.avatar_url, aud: coreId },
		SESSION_TTL_S,
	)
	return json({ ticket })
}
