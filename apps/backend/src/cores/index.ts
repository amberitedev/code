import { connect } from 'cloudflare:sockets'

import { findUser, requireUser, tokenFrom } from '../common/auth'
import { digest, randomId } from '../common/crypto'
import {
	ApiError,
	invalid,
	json,
	readJson,
	routePath,
	stringField,
	unauthorized,
} from '../common/http'
import { addNotification } from '../common/notifications'
import type { Env } from '../common/types'
import { blockedEitherWay } from '../social/preferences'

// Core pairing. A Core registers the code it is showing, an account claims that code in the App,
// and the Core then confirms the claim to receive its own key. After that the backend only keeps
// the account's list of Cores and their addresses, and tests on request whether a Core can be
// reached from the internet.
//
// A Core's owner can invite other accounts to manage its servers. The Core keeps who has which
// role on which server; the backend only keeps which accounts may reach the Core, so their App is
// told its address too, and delivers the invite.

const CODE_TTL_MS = 15 * 60_000
// Expired codes are kept this long so a late guess is told "expired" instead of "wrong".
const EXPIRED_GRACE_MS = 60 * 60_000
const MAX_WRONG_GUESSES = 5
const GUESS_WINDOW_MS = 10 * 60_000
// The Core's alphabet: no i, o, 0 or 1.
const CODE_PATTERN = /^[a-hj-np-z2-9]{8}$/
const FINGERPRINT_PATTERN = /^[0-9a-f]{64}$/
// Outside tests one Core may ask for within `CHECK_WINDOW_MS`.
const MAX_CHECKS = 30
const CHECK_WINDOW_MS = 10 * 60_000
const CHECK_TIMEOUT_MS = 5_000
// Invites one Core may send within `INVITE_WINDOW_MS`.
const MAX_INVITES = 30
const INVITE_WINDOW_MS = 10 * 60_000

interface CodeRow {
	code: string
	core_id: string
	address: string
	cert_fingerprint: string | null
	expires: string
	claimed_by: string | null
}

interface CoreRow {
	id: string
	owner_id: string
	name: string
	address: string
	cert_fingerprint: string | null
	public_address: string | null
	version: string | null
	created: string
	last_seen: string
}

function isPrivateHost(hostname: string): boolean {
	const host = hostname.replace(/^\[|\]$/g, '').toLowerCase()
	if (host === 'localhost' || host === '::1') return true
	const ipv4 = host.match(/^(\d{1,3})\.(\d{1,3})\.\d{1,3}\.\d{1,3}$/)
	if (ipv4) {
		const [first, second] = [Number(ipv4[1]), Number(ipv4[2])]
		return (
			first === 127 ||
			first === 10 ||
			(first === 172 && second >= 16 && second <= 31) ||
			(first === 192 && second === 168) ||
			(first === 169 && second === 254)
		)
	}
	// IPv6 unique local (fc00::/7) and link local (fe80::/10).
	return host.includes(':') && /^(f[cd]|fe[89ab])/.test(host)
}

/** A Core address must be HTTPS unless it is loopback or on a private network. */
function coreAddress(body: Record<string, unknown>, key = 'address'): string {
	const value = stringField(body, key, 512)
	let url: URL
	try {
		url = new URL(value)
	} catch {
		throw new ApiError(400, 'invalid_address', 'The Core address is not a valid URL')
	}
	const allowed =
		url.protocol === 'https:' || (url.protocol === 'http:' && isPrivateHost(url.hostname))
	if (!allowed || url.username || url.password)
		throw new ApiError(
			400,
			'invalid_address',
			'A Core address must use HTTPS unless it is on this computer or a private network',
		)
	return url.origin + url.pathname.replace(/\/$/, '')
}

/** The SHA-256 of the Core's certificate, or null from a Core that does not serve HTTPS yet. */
function certFingerprint(body: Record<string, unknown>): string | null {
	if (body.cert_fingerprint == null) return null
	const value = stringField(body, 'cert_fingerprint', 64)
	if (!FINGERPRINT_PATTERN.test(value))
		throw new ApiError(400, 'invalid_input', 'Invalid cert_fingerprint')
	return value
}

function pairingCode(body: Record<string, unknown>): string {
	return stringField(body, 'code', 32)
		.replace(/[^a-z0-9]/gi, '')
		.toLowerCase()
}

/** A Core as `userId` sees it. `owned` is false for a Core they were only invited to. */
function publicCore(row: CoreRow, userId: string) {
	return {
		id: row.id,
		name: row.name,
		address: row.address,
		cert_fingerprint: row.cert_fingerprint,
		public_address: row.public_address,
		version: row.version,
		owned: row.owner_id === userId,
		created: row.created,
		last_seen: row.last_seen,
	}
}

/** Core, unowned: publish the code it is showing. */
async function registerCode(request: Request, env: Env) {
	const body = await readJson(request)
	const code = pairingCode(body)
	if (!CODE_PATTERN.test(code)) throw new ApiError(400, 'invalid_input', 'Invalid code')
	const coreId = stringField(body, 'core_id', 64)
	const address = coreAddress(body)
	const fingerprint = certFingerprint(body)
	const now = Date.now()
	const nowIso = new Date(now).toISOString()

	await env.DB.prepare('DELETE FROM core_pairing_codes WHERE expires < ?')
		.bind(new Date(now - EXPIRED_GRACE_MS).toISOString())
		.run()
	if (await env.DB.prepare('SELECT 1 FROM cores WHERE id = ?').bind(coreId).first())
		throw new ApiError(409, 'already_paired', 'This Core is already paired to an account')
	// A Core may replace its own code. Another Core's live code is never overwritten.
	const taken = await env.DB.prepare(
		'SELECT 1 FROM core_pairing_codes WHERE code = ? AND core_id != ? AND expires > ?',
	)
		.bind(code, coreId, nowIso)
		.first()
	if (taken) throw new ApiError(409, 'code_taken', 'This code is in use')
	const expires = new Date(now + CODE_TTL_MS).toISOString()
	await env.DB.batch([
		env.DB.prepare('DELETE FROM core_pairing_codes WHERE code = ? OR core_id = ?').bind(
			code,
			coreId,
		),
		env.DB.prepare(
			'INSERT INTO core_pairing_codes (code,core_id,address,cert_fingerprint,expires) VALUES (?,?,?,?,?)',
		).bind(code, coreId, address, fingerprint, expires),
	])
	return json({ expires })
}

/**
 * App: claim the code a Core is showing. Returns where that Core is, and the certificate to trust
 * there, so the App can finish with the Core itself.
 */
async function claimCode(request: Request, env: Env) {
	const user = await requireUser(request, env)
	const code = pairingCode(await readJson(request))
	const now = Date.now()
	const nowIso = new Date(now).toISOString()

	const attempts = await env.DB.prepare(
		'SELECT failures,window_start FROM core_pairing_attempts WHERE user_id = ?',
	)
		.bind(user.id)
		.first<{ failures: number; window_start: string }>()
	const windowEnds = attempts ? Date.parse(attempts.window_start) + GUESS_WINDOW_MS : 0
	const failures = attempts && windowEnds > now ? attempts.failures : 0
	if (failures >= MAX_WRONG_GUESSES)
		throw new ApiError(
			429,
			'too_many_attempts',
			`Too many wrong codes. Try again in ${Math.ceil((windowEnds - now) / 60_000)} minutes.`,
		)

	const row = await env.DB.prepare('SELECT * FROM core_pairing_codes WHERE code = ?')
		.bind(code)
		.first<CodeRow>()
	if (!row) {
		await env.DB.prepare(
			'INSERT INTO core_pairing_attempts (user_id,failures,window_start) VALUES (?,1,?) ON CONFLICT(user_id) DO UPDATE SET failures = ?, window_start = ?',
		)
			.bind(user.id, nowIso, failures + 1, failures ? (attempts?.window_start ?? nowIso) : nowIso)
			.run()
		throw new ApiError(404, 'wrong_code', 'That code does not match any Core')
	}
	if (row.expires <= nowIso)
		throw new ApiError(410, 'code_expired', 'That code has expired. Use the new one on your Core.')
	const claimed = await env.DB.prepare(
		'UPDATE core_pairing_codes SET claimed_by = ? WHERE code = ? AND (claimed_by IS NULL OR claimed_by = ?) RETURNING code',
	)
		.bind(user.id, code, user.id)
		.first()
	if (!claimed)
		throw new ApiError(409, 'already_paired', 'That Core is being paired to another account')
	return json({
		core_id: row.core_id,
		address: row.address,
		cert_fingerprint: row.cert_fingerprint,
	})
}

/**
 * Core: check that `user_id` claimed this Core's code. On success the Core joins that account's
 * list and gets the key it uses from now on. Only the key's hash is stored.
 */
async function confirmClaim(request: Request, env: Env) {
	const body = await readJson(request)
	const code = pairingCode(body)
	const coreId = stringField(body, 'core_id', 64)
	const userId = stringField(body, 'user_id', 64)
	const name = stringField(body, 'name', 64)
	const address = coreAddress(body)
	const fingerprint = certFingerprint(body)
	const now = new Date().toISOString()

	const row = await env.DB.prepare(
		'SELECT * FROM core_pairing_codes WHERE code = ? AND core_id = ? AND expires > ?',
	)
		.bind(code, coreId, now)
		.first<CodeRow>()
	if (!row || row.claimed_by !== userId)
		throw new ApiError(403, 'not_claimed', 'This code was not claimed by that account')

	const key = `amc_${randomId(48)}`
	// Repeating a confirmation whose answer was lost issues a fresh key to the same owner. A Core
	// that belongs to someone else is never taken over.
	const stored = await env.DB.prepare(
		'INSERT INTO cores (id,owner_id,name,address,cert_fingerprint,key_hash,created,last_seen) VALUES (?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET key_hash = excluded.key_hash, address = excluded.address, cert_fingerprint = excluded.cert_fingerprint, public_address = NULL, last_seen = excluded.last_seen WHERE owner_id = excluded.owner_id RETURNING id',
	)
		.bind(coreId, userId, name, address, fingerprint, await digest(key), now, now)
		.first()
	if (!stored)
		throw new ApiError(409, 'already_paired', 'This Core is already paired to another account')
	return json({ key })
}

/**
 * Core, paired: report where it is now. Called every time the Core starts and whenever its address
 * on the internet changes. A Core paired before Cores served HTTPS reports its certificate here.
 */
async function reportAddress(request: Request, env: Env) {
	const key = tokenFrom(request)
	if (!key.startsWith('amc_')) unauthorized()
	const body = await readJson(request)
	const address = coreAddress(body)
	const publicAddress = body.public_address == null ? null : coreAddress(body, 'public_address')
	if (publicAddress && !publicAddress.startsWith('https:'))
		throw new ApiError(400, 'invalid_address', 'A public Core address must use HTTPS')
	const version = body.version == null ? null : stringField(body, 'version', 32)
	const row = await env.DB.prepare(
		'UPDATE cores SET address = ?, public_address = ?, cert_fingerprint = COALESCE(?, cert_fingerprint), version = COALESCE(?, version), last_seen = ? WHERE key_hash = ? RETURNING *',
	)
		.bind(
			address,
			publicAddress,
			certFingerprint(body),
			version,
			new Date().toISOString(),
			await digest(key),
		)
		.first<CoreRow>()
	if (!row) unauthorized()
	return json({ id: row.id, owner_id: row.owner_id })
}

/** Try to open a TCP connection. `detail` says how it failed, ready to show. */
async function tcpConnects(
	hostname: string,
	port: number,
): Promise<{ reachable: boolean; detail: string }> {
	let socket: Socket | undefined
	let timer: ReturnType<typeof setTimeout> | undefined
	try {
		socket = connect({ hostname, port })
		const timedOut = new Promise<false>((resolve) => {
			timer = setTimeout(() => resolve(false), CHECK_TIMEOUT_MS)
		})
		const opened = await Promise.race([socket.opened.then(() => true as const), timedOut])
		return opened
			? { reachable: true, detail: '' }
			: {
					reachable: false,
					detail: `no answer within ${CHECK_TIMEOUT_MS / 1000} seconds, the connection was dropped on the way`,
				}
	} catch (error) {
		const reason = error instanceof Error ? error.message : String(error)
		return { reachable: false, detail: `the connection was refused or reset (${reason})` }
	} finally {
		clearTimeout(timer)
		void socket?.close().catch(() => undefined)
	}
}

/**
 * Core, paired: test whether a port on this Core can be reached from the internet. The connection
 * only ever goes to the address this request came from, so it cannot be used to probe anyone else.
 * `outside` is false when that address is a private one: this backend is then on the Core's own
 * network (local development) and its answer says nothing about the internet.
 */
async function checkReachability(request: Request, env: Env) {
	const key = tokenFrom(request)
	if (!key.startsWith('amc_')) unauthorized()
	const port = (await readJson(request)).port
	if (typeof port !== 'number' || !Number.isInteger(port) || port < 1 || port > 65535)
		throw new ApiError(400, 'invalid_input', 'Invalid port')

	const now = Date.now()
	const nowIso = new Date(now).toISOString()
	const windowStart = new Date(now - CHECK_WINDOW_MS).toISOString()
	const row = await env.DB.prepare(
		'UPDATE cores SET checks = CASE WHEN checks_window IS NULL OR checks_window < ?1 THEN 1 ELSE checks + 1 END, checks_window = CASE WHEN checks_window IS NULL OR checks_window < ?1 THEN ?2 ELSE checks_window END, last_seen = ?2 WHERE key_hash = ?3 RETURNING checks',
	)
		.bind(windowStart, nowIso, await digest(key))
		.first<{ checks: number }>()
	if (!row) unauthorized()
	if (row.checks > MAX_CHECKS)
		throw new ApiError(429, 'too_many_checks', 'Too many reachability tests. Try again later.')

	// Cloudflare sets CF-Connecting-IP itself, so a caller cannot choose it. The local Worker only
	// listens on this computer and passes the header through untouched, so it is not read there.
	const local = env.LOCAL_DEV === 'true'
	const address = local ? '127.0.0.1' : request.headers.get('CF-Connecting-IP')
	if (!address) throw new ApiError(500, 'internal_error', 'The caller address is unknown')
	const outside = !isPrivateHost(address)
	// A private caller is only ever tested in local development, where it exercises this route.
	if (!outside && !local) return json({ address, outside, reachable: false, detail: '' })
	return json({ address, outside, ...(await tcpConnects(address, port)) })
}

/** The Core a request's key belongs to. */
async function requireCore(request: Request, env: Env): Promise<CoreRow> {
	const key = tokenFrom(request)
	if (!key.startsWith('amc_')) unauthorized()
	const core = await env.DB.prepare('SELECT * FROM cores WHERE key_hash = ?')
		.bind(await digest(key))
		.first<CoreRow>()
	if (!core) unauthorized()
	return core
}

/** One account's invite to one server of one Core. Binds: account, Core, server. */
const INVITE_FILTER =
	"user_id = ? AND json_extract(body,'$.type') = 'server_invite' AND json_extract(body,'$.core_id') = ? AND json_extract(body,'$.server_id') = ?"

/**
 * Core, paired: an account was invited to one of its servers. The account may reach this Core from
 * now on, and gets the invite as a notification to accept in the App. `user` is an id or a
 * username; the answer says who it is, so the Core can show their name. `invited_by` must be the
 * Core's owner or an account that can already reach it.
 */
async function grantAccess(request: Request, env: Env) {
	const core = await requireCore(request, env)
	const body = await readJson(request)
	const serverId = stringField(body, 'server_id', 64)
	const serverName = stringField(body, 'server_name', 100)
	const role = stringField(body, 'role', 16)
	const invitedBy = stringField(body, 'invited_by', 64)
	const username = stringField(body, 'user', 64)
	if (
		invitedBy !== core.owner_id &&
		!(await env.DB.prepare('SELECT 1 FROM core_access WHERE core_id = ? AND user_id = ?')
			.bind(core.id, invitedBy)
			.first())
	)
		throw new ApiError(403, 'forbidden', 'Only someone with access to this Core can invite')

	// Counted before the account is looked up, so a Core cannot use this to test names either.
	const now = Date.now()
	await env.DB.prepare('DELETE FROM auth_attempts WHERE expires < ?').bind(now).run()
	const sent = await env.DB.prepare(
		'INSERT INTO auth_attempts (key,count,expires) VALUES (?1,1,?2) ON CONFLICT(key) DO UPDATE SET count = CASE WHEN expires < ?3 THEN 1 ELSE count + 1 END, expires = CASE WHEN expires < ?3 THEN excluded.expires ELSE expires END RETURNING count',
	)
		.bind(`core-access:${core.id}`, now + INVITE_WINDOW_MS, now)
		.first<{ count: number }>()
	if (sent && sent.count > MAX_INVITES)
		throw new ApiError(429, 'too_many_invites', 'Too many invites. Try again later.')

	// Answers like a friend request does, for an unknown name and for a block.
	const user = await findUser(env, username)
	if (!user) throw new ApiError(404, 'not_found', 'Not found')
	if (user.id === core.owner_id) invalid('The owner already has full access')
	if (
		(await blockedEitherWay(env, user.id, invitedBy)) ||
		(await blockedEitherWay(env, user.id, core.owner_id))
	)
		invalid("You've blocked the other user or have been blocked by them!")
	await env.DB.batch([
		env.DB.prepare(
			'INSERT OR IGNORE INTO core_access (core_id,user_id,created) VALUES (?,?,?)',
		).bind(core.id, user.id, new Date().toISOString()),
		// Access may already exist for another server. Every invite still gets a fresh notification.
		env.DB.prepare(`DELETE FROM notifications WHERE ${INVITE_FILTER}`).bind(
			user.id,
			core.id,
			serverId,
		),
	])
	await addNotification(env, user.id, {
		type: 'server_invite',
		core_id: core.id,
		server_id: serverId,
		server_name: serverName,
		role,
		invited_by: invitedBy,
	})
	return json({ id: user.id, username: user.username, avatar_url: user.avatar_url })
}

/**
 * Core, paired: an account lost its access or its invite to one server. Its invite is withdrawn,
 * and unless `keep_core` says it still has another server there, so is its way to this Core.
 */
async function revokeAccess(request: Request, env: Env) {
	const core = await requireCore(request, env)
	const body = await readJson(request)
	const userId = stringField(body, 'user_id', 64)
	await env.DB.prepare(`DELETE FROM notifications WHERE ${INVITE_FILTER}`)
		.bind(userId, core.id, stringField(body, 'server_id', 64))
		.run()
	if (body.keep_core !== true)
		await env.DB.prepare('DELETE FROM core_access WHERE core_id = ? AND user_id = ?')
			.bind(core.id, userId)
			.run()
	return new Response(null, { status: 204 })
}

export async function handleCores(request: Request, env: Env): Promise<Response | null> {
	const path = routePath(request),
		method = request.method
	if (path === '/core/pairing' && method === 'POST') return registerCode(request, env)
	if (path === '/core/pairing/confirm' && method === 'POST') return confirmClaim(request, env)
	if (path === '/core/address' && method === 'POST') return reportAddress(request, env)
	if (path === '/core/reachability' && method === 'POST') return checkReachability(request, env)
	if (path === '/core/access' && method === 'POST') return grantAccess(request, env)
	if (path === '/core/access' && method === 'DELETE') return revokeAccess(request, env)
	if (path === '/cores/pair' && method === 'POST') return claimCode(request, env)
	if (path === '/cores' && method === 'GET') {
		// The account's own Cores, then the ones it was invited to. Nobody else is told where a
		// Core is.
		const user = await requireUser(request, env)
		const rows = await env.DB.prepare(
			'SELECT * FROM cores WHERE owner_id = ?1 OR id IN (SELECT core_id FROM core_access WHERE user_id = ?1) ORDER BY owner_id = ?1 DESC, created',
		)
			.bind(user.id)
			.all<CoreRow>()
		return json(rows.results.map((row) => publicCore(row, user.id)))
	}
	const core = path.match(/^\/cores\/([^/]+)$/)
	if (core && method === 'DELETE') {
		const user = await requireUser(request, env)
		const deleted = await env.DB.prepare('DELETE FROM cores WHERE id = ? AND owner_id = ?')
			.bind(decodeURIComponent(core[1]), user.id)
			.run()
		if (!deleted.meta.changes) throw new ApiError(404, 'not_found', 'Core not found')
		return new Response(null, { status: 204 })
	}
	return null
}
