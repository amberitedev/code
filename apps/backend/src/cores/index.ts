import {
	type AccessDirectory,
	type AccessGrant,
	type AccessRevocation,
	type EnteredCode,
	type ForgottenServer,
	type ListedCore,
	type NewSignInCode,
	type SignInStatus,
	type StateReport,
	type VisibleServer,
	cores,
} from '@amberite/contracts'

import { findUser, requireUser, tokenFrom } from '../common/auth'
import { isPrivateHost } from '../common/address'
import { digest, randomId } from '../common/crypto'
import { ApiError, invalid, objectBody, stringField, unauthorized } from '../common/http'
import { addNotification } from '../common/notifications'
import { type Route, noBody, route } from '../common/router'
import type { Env } from '../common/types'
import { blockedEitherWay } from '../social/preferences'
import { forgetServer, sweepFiles } from '../sharing'
import { MAX_SERVER_NAME, writeServers } from './servers'
import { waitForApproval } from './sign-in'
import { MAX_REPORT_BODY, parseStorageReport, reportStorage } from './storage'
import { issueTicket } from './tickets'

// Core sign-in. A signed-out Core asks for a code and the key it will use, shows the code to its
// owner, and waits on a WebSocket that approval closes (`sign-in.ts`), polling only to read the
// answer. The owner enters the code in the App, which approves it with their session; from then
// on the key works and the Core is listed under that account. The App never talks to the Core
// during sign-in and the account session never leaves the App. After that the backend only
// keeps the account's list of Cores and their addresses. A Core reports its state on start and on
// change, never on a timer, and a report that changes nothing writes nothing: D1 writes are the
// free tier's scarcest resource.
//
// A Core's owner can invite other accounts to manage its servers. The Core keeps who has which
// role on which server and is the only authority on it; the backend keeps a copy of which accounts
// may reach the Core, so their App is told its address too and can get tickets for it, and
// delivers the invite. Only the owner invites.

const CODE_TTL_MS = 15 * 60_000
const POLL_INTERVAL_S = 5
// Expired codes are kept this long so a late guess is told "expired" instead of "wrong".
const EXPIRED_GRACE_MS = 60 * 60_000
// Wrong or expired codes one account may try within `GUESS_WINDOW_MS`. Right codes do not count.
const MAX_WRONG_GUESSES = 5
const GUESS_WINDOW_MS = 10 * 60_000
// Code requests one Core, and one caller address, may make within `REQUEST_WINDOW_MS`.
const MAX_CORE_REQUESTS = 10
const MAX_ADDRESS_REQUESTS = 60
const REQUEST_WINDOW_MS = 10 * 60_000
// The code's alphabet: no i, o, 0 or 1, so it reads the same in any font.
const CODE_ALPHABET = 'abcdefghjklmnpqrstuvwxyz23456789'
const FINGERPRINT_PATTERN = /^[0-9a-f]{64}$/
// Invites one Core may send within `INVITE_WINDOW_MS`.
const MAX_INVITES = 30
const INVITE_WINDOW_MS = 10 * 60_000
// Accounts one Core's access directory may hold. Core refuses an invite past it, with
// `MAX_DIRECTORY` in `apps/core/src/api/access.rs`; the two must stay equal.
const MAX_DIRECTORY = 1000
const MAX_SERVERS = 100

interface PendingRow {
	code: string
	core_id: string
	name: string
	address: string
	cert_fingerprint: string | null
	key_hash: string
	expires: string
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

/** A Core as `userId` sees it. `owned` is false for a Core they were only invited to. */
function publicCore(row: CoreRow, userId: string): ListedCore {
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

function parseEnteredCode(raw: unknown): EnteredCode {
	return { code: stringField(objectBody(raw), 'code', 32) }
}

/** The code as the user typed it: the dash and case do not matter. */
function authCode({ code }: EnteredCode): string {
	return code.replace(/[^a-z0-9]/gi, '').toLowerCase()
}

function generateCode(): string {
	return Array.from(
		crypto.getRandomValues(new Uint8Array(8)),
		(byte) => CODE_ALPHABET[byte & 31],
	).join('')
}

/** Where a request came from. Cloudflare sets the header itself, so a caller cannot choose it. */
function callerAddress(request: Request, env: Env): string {
	return env.LOCAL_DEV === 'true' ? '127.0.0.1' : (request.headers.get('CF-Connecting-IP') ?? '')
}

/**
 * Count one event under `scope` and return the count in the current window, in one statement so
 * concurrent requests cannot slip past a limit. A window starts with the first event after the
 * previous window ended.
 */
async function countInWindow(env: Env, scope: string, windowMs: number, now: number) {
	const row = await env.DB.prepare(
		'INSERT INTO core_auth_limits (scope,count,window_start) VALUES (?1,1,?2) ON CONFLICT(scope) DO UPDATE SET count = CASE WHEN window_start <= ?3 THEN 1 ELSE count + 1 END, window_start = CASE WHEN window_start <= ?3 THEN ?2 ELSE window_start END RETURNING count, window_start',
	)
		.bind(scope, new Date(now).toISOString(), new Date(now - windowMs).toISOString())
		.first<{ count: number; window_start: string }>()
	if (!row) throw new ApiError(500, 'internal_error', 'Could not count the request')
	return { count: row.count, windowEnds: Date.parse(row.window_start) + windowMs }
}

function parseNewSignInCode(raw: unknown): NewSignInCode {
	const body = objectBody(raw)
	return {
		core_id: stringField(body, 'core_id', 64),
		name: stringField(body, 'name', 64),
		address: coreAddress(body),
		cert_fingerprint: certFingerprint(body),
	}
}

/**
 * Core, signed out: ask for a code to show its owner, and the key it will use once the owner
 * approves. Only the key's hash is stored, and the key does nothing until approval. A Core has one
 * pending code at a time; asking again replaces it.
 */
async function requestCode(request: Request, env: Env, body: NewSignInCode) {
	const { core_id: coreId, name, address, cert_fingerprint: fingerprint } = body
	const now = Date.now()

	// Checked before counting: a listed Core keeps asking until it is removed in the App, and must
	// not use up the requests it needs right after.
	if (await env.DB.prepare('SELECT 1 FROM cores WHERE id = ?').bind(coreId).first())
		throw new ApiError(
			409,
			'already_signed_in',
			'This Core is still listed under an account. Remove it in the App under Settings, Cores.',
		)

	const byCore = await countInWindow(env, `code:${coreId}`, REQUEST_WINDOW_MS, now)
	const byAddress = await countInWindow(
		env,
		`ip:${callerAddress(request, env)}`,
		REQUEST_WINDOW_MS,
		now,
	)
	if (byCore.count > MAX_CORE_REQUESTS || byAddress.count > MAX_ADDRESS_REQUESTS)
		throw new ApiError(
			429,
			'too_many_requests',
			'Too many sign-in codes were asked for. Try again later.',
		)

	await env.DB.batch([
		env.DB.prepare('DELETE FROM core_auth_codes WHERE expires < ?').bind(
			new Date(now - EXPIRED_GRACE_MS).toISOString(),
		),
		env.DB.prepare('DELETE FROM core_auth_limits WHERE window_start < ?').bind(
			new Date(now - Math.max(GUESS_WINDOW_MS, REQUEST_WINDOW_MS)).toISOString(),
		),
	])

	const key = `amc_${randomId(48)}`
	const keyHash = await digest(key)
	const expires = new Date(now + CODE_TTL_MS).toISOString()
	// A code still held by another Core is not reused; a new one is drawn instead.
	for (let attempt = 0; ; attempt++) {
		const code = generateCode()
		try {
			await env.DB.batch([
				env.DB.prepare('DELETE FROM core_auth_codes WHERE core_id = ?').bind(coreId),
				env.DB.prepare(
					'INSERT INTO core_auth_codes (code,core_id,name,address,cert_fingerprint,key_hash,expires) VALUES (?,?,?,?,?,?,?)',
				).bind(code, coreId, name, address, fingerprint, keyHash, expires),
			])
		} catch (error) {
			if (attempt < 3) continue
			throw error
		}
		return { code, key, expires_in: CODE_TTL_MS / 1000, poll_interval: POLL_INTERVAL_S }
	}
}

/**
 * Core, signed out: has the owner approved yet? Sent with the key from `requestCode`. Approval is
 * what makes that key a Core's key, so an approved key is recognised here like on every other Core
 * route. 410 tells the Core its code is gone and it should ask for a new one.
 */
async function pollCode(request: Request, env: Env): Promise<SignInStatus> {
	const key = tokenFrom(request)
	if (!key.startsWith('amc_')) unauthorized()
	const keyHash = await digest(key)
	const core = await env.DB.prepare(
		'SELECT u.id, u.username FROM cores c JOIN users u ON u.id = c.owner_id WHERE c.key_hash = ?',
	)
		.bind(keyHash)
		.first<{ id: string; username: string }>()
	if (core) return { status: 'approved', account: core }
	const pending = await env.DB.prepare('SELECT expires FROM core_auth_codes WHERE key_hash = ?')
		.bind(keyHash)
		.first<{ expires: string }>()
	if (pending && pending.expires > new Date().toISOString()) return { status: 'pending' }
	throw new ApiError(410, 'code_expired', 'That code has expired')
}

/**
 * The pending Core behind a code the signed-in account typed. Only wrong and expired codes count
 * towards the account's limit, so a right code never clears it.
 */
async function pendingCore(env: Env, userId: string, code: string): Promise<PendingRow> {
	const now = Date.now()
	const scope = `approve:${userId}`
	const tries = await env.DB.prepare(
		'SELECT count, window_start FROM core_auth_limits WHERE scope = ?',
	)
		.bind(scope)
		.first<{ count: number; window_start: string }>()
	const windowEnds = tries ? Date.parse(tries.window_start) + GUESS_WINDOW_MS : 0
	if (tries && windowEnds > now && tries.count >= MAX_WRONG_GUESSES)
		throw new ApiError(
			429,
			'too_many_attempts',
			`Too many wrong codes. Try again in ${Math.ceil((windowEnds - now) / 60_000)} minutes.`,
		)
	const row = await env.DB.prepare('SELECT * FROM core_auth_codes WHERE code = ?')
		.bind(code)
		.first<PendingRow>()
	const expired = row && row.expires <= new Date(now).toISOString()
	if (!row || expired) await countInWindow(env, scope, GUESS_WINDOW_MS, now)
	if (!row) throw new ApiError(404, 'wrong_code', 'That code does not match any Core')
	if (expired)
		throw new ApiError(410, 'code_expired', 'That code has expired. Use the new one on your Core.')
	return row
}

/** App: which machine is behind this code, so the user can check before approving. */
async function lookupCode(request: Request, env: Env, body: () => Promise<EnteredCode>) {
	const user = await requireUser(request, env)
	const row = await pendingCore(env, user.id, authCode(await body()))
	return { name: row.name }
}

/**
 * App: approve the Core showing this code. It joins the account's list and the key it holds starts
 * working. The insert reads the pending row itself, so a code replaced or expired since the lookup
 * cannot store a key the Core no longer holds. A Core that belongs to someone else is never taken
 * over.
 */
async function approveCode(request: Request, env: Env, body: () => Promise<EnteredCode>) {
	const user = await requireUser(request, env)
	const row = await pendingCore(env, user.id, authCode(await body()))
	const now = new Date().toISOString()
	const stored = await env.DB.prepare(
		'INSERT INTO cores (id,owner_id,name,address,cert_fingerprint,key_hash,created,last_seen) SELECT core_id, ?1, name, address, cert_fingerprint, key_hash, ?2, ?2 FROM core_auth_codes WHERE code = ?3 AND expires > ?2 ON CONFLICT(id) DO UPDATE SET key_hash = excluded.key_hash, name = excluded.name, address = excluded.address, cert_fingerprint = excluded.cert_fingerprint, public_address = NULL, last_seen = excluded.last_seen WHERE owner_id = excluded.owner_id RETURNING id, name',
	)
		.bind(user.id, now, row.code)
		.first<{ id: string; name: string }>()
	if (!stored) {
		const pending = await env.DB.prepare(
			'SELECT 1 FROM core_auth_codes WHERE code = ? AND expires > ?',
		)
			.bind(row.code, now)
			.first()
		if (!pending)
			throw new ApiError(
				410,
				'code_expired',
				'That code has expired. Use the new one on your Core.',
			)
		throw new ApiError(409, 'already_signed_in', 'That Core belongs to another account')
	}
	await env.DB.prepare('DELETE FROM core_auth_codes WHERE code = ?').bind(row.code).run()
	// A Core that cannot be woken still learns of the approval when its code would expire.
	await env.CORE_SIGN_IN.getByName(stored.id)
		.approved()
		.catch((error: unknown) => console.error('Could not wake the Core', error))
	return stored
}

/** What a state report may change on a Core's row. */
const STATE_FIELDS = ['address', 'public_address', 'cert_fingerprint', 'version'] as const
type CoreState = Pick<CoreRow, (typeof STATE_FIELDS)[number]>

/** Store a Core's reported state, writing only when a field changed. */
async function writeState(env: Env, core: CoreRow, next: CoreState) {
	if (STATE_FIELDS.every((field) => core[field] === next[field])) return
	await env.DB.prepare(
		'UPDATE cores SET address = ?, public_address = ?, cert_fingerprint = ?, version = ?, last_seen = ? WHERE id = ?',
	)
		.bind(
			next.address,
			next.public_address,
			next.cert_fingerprint,
			next.version,
			new Date().toISOString(),
			core.id,
		)
		.run()
}

/** An address the new Core serves its API on. It has no plain HTTP listener. */
function httpsAddress(body: Record<string, unknown>, key: string): string {
	const address = coreAddress(body, key)
	if (!address.startsWith('https:'))
		throw new ApiError(400, 'invalid_address', 'A Core address must use HTTPS')
	return address
}

function parseStateReport(raw: unknown): StateReport {
	const body = objectBody(raw)
	const fingerprint = certFingerprint(body)
	if (!fingerprint) invalid('Invalid cert_fingerprint')
	return {
		address: httpsAddress(body, 'address'),
		public_address: body.public_address == null ? null : httpsAddress(body, 'public_address'),
		cert_fingerprint: fingerprint,
		version: stringField(body, 'version', 32),
		servers: parseServers(body.servers),
	}
}

function parseServers(servers: unknown): VisibleServer[] {
	if (!Array.isArray(servers) || servers.length > MAX_SERVERS) invalid('Invalid servers')
	return servers.map((server: unknown) => {
		const body = objectBody(server)
		return {
			id: stringField(body, 'id', 64),
			name: stringField(body, 'name', MAX_SERVER_NAME),
			address: stringField(body, 'address', 261),
			shared_instance_id: stringField(body, 'shared_instance_id', 64),
		}
	})
}

/**
 * Core, signed in: its whole state, sent once on start and again when any of it changes: where the
 * App reaches it, the certificate the App pins, its version, and the servers it shows to friends.
 * A report that changes nothing writes nothing. A Core removed from its account gets 410
 * `core_removed` and un-pairs.
 */
async function reportState(request: Request, env: Env, body: () => Promise<StateReport>) {
	const core = await requireListedCore(request, env)
	const report = await body()
	await writeState(env, core, {
		address: report.address,
		public_address: report.public_address,
		cert_fingerprint: report.cert_fingerprint,
		version: report.version,
	})
	await writeServers(env, core, report.servers)
}

/** The Core a request's key belongs to, or null for a key no Core has. */
async function findCore(request: Request, env: Env): Promise<CoreRow | null> {
	const key = tokenFrom(request)
	if (!key.startsWith('amc_')) unauthorized()
	return env.DB.prepare('SELECT * FROM cores WHERE key_hash = ?')
		.bind(await digest(key))
		.first<CoreRow>()
}

/**
 * The Core a request's key belongs to. A key that belonged to a Core that was since removed from
 * its account gets 410, so the Core un-pairs on that answer and on no other.
 */
export async function requireListedCore(request: Request, env: Env): Promise<CoreRow> {
	const core = await findCore(request, env)
	if (!core)
		throw new ApiError(
			410,
			'core_removed',
			'This Core was removed from its account. Pair it again.',
		)
	return core
}

/** One account's invite to one server of one Core. Binds: account, Core, server. */
const INVITE_FILTER =
	"user_id = ? AND json_extract(body,'$.type') = 'server_invite' AND json_extract(body,'$.core_id') = ? AND json_extract(body,'$.server_id') = ?"

function parseAccessGrant(raw: unknown): AccessGrant {
	const body = objectBody(raw)
	return {
		user: stringField(body, 'user', 64),
		server_id: stringField(body, 'server_id', 64),
		server_name: stringField(body, 'server_name', MAX_SERVER_NAME),
		role: body.role === 'viewer' || body.role === 'editor' ? body.role : invalid('Invalid role'),
	}
}

/**
 * Core, signed in: its owner invited an account to one of its servers. The account may reach this
 * Core from now on, and gets the invite as a notification. `user` is an id or a username; the
 * answer says who it is, so the Core can show their name. Only the owner invites, so the invite
 * is always from the owner.
 */
async function grantAccess(request: Request, env: Env, body: () => Promise<AccessGrant>) {
	const core = await requireListedCore(request, env)
	const { server_id: serverId, server_name: serverName, role, user: username } = await body()

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
	if (await blockedEitherWay(env, user.id, core.owner_id))
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
		invited_by: core.owner_id,
	})
	return { id: user.id, username: user.username, avatar_url: user.avatar_url }
}

function parseAccessDirectory(raw: unknown): AccessDirectory {
	const userIds = objectBody(raw).user_ids
	if (!Array.isArray(userIds) || userIds.length > MAX_DIRECTORY) invalid('Invalid user_ids')
	return {
		user_ids: userIds.map((id: unknown) =>
			typeof id === 'string' && id.length > 0 && id.length <= 64 ? id : invalid('Invalid user_ids'),
		),
	}
}

/**
 * Core, signed in: the whole access directory, everyone with a role on any of its servers. Sent on
 * every start and reconnect, it replaces the backend's copy, so a missed grant or removal cannot
 * leave the two apart. Only rows that change are written. Unknown accounts and the owner are
 * skipped.
 */
async function replaceAccess(request: Request, env: Env, body: () => Promise<AccessDirectory>) {
	const core = await requireListedCore(request, env)
	const ids = JSON.stringify((await body()).user_ids)
	await env.DB.batch([
		env.DB.prepare(
			'DELETE FROM core_access WHERE core_id = ?1 AND user_id NOT IN (SELECT value FROM json_each(?2))',
		).bind(core.id, ids),
		env.DB.prepare(
			'INSERT OR IGNORE INTO core_access (core_id,user_id,created) SELECT ?1, id, ?3 FROM users WHERE id IN (SELECT value FROM json_each(?2)) AND id != ?4',
		).bind(core.id, ids, new Date().toISOString(), core.owner_id),
	])
}

/**
 * Core, signed in: an account lost its access or its invite to one server. Its invite is withdrawn,
 * and unless `keep_core` says it still has another server there, so is its way to this Core.
 */
async function revokeAccess(request: Request, env: Env, body: () => Promise<AccessRevocation>) {
	const core = await requireListedCore(request, env)
	const revocation = await body()
	const userId = revocation.user_id
	await env.DB.prepare(`DELETE FROM notifications WHERE ${INVITE_FILTER}`)
		.bind(userId, core.id, revocation.server_id)
		.run()
	if (!revocation.keep_core)
		await env.DB.prepare('DELETE FROM core_access WHERE core_id = ? AND user_id = ?')
			.bind(core.id, userId)
			.run()
}

function parseAccessRevocation(raw: unknown): AccessRevocation {
	const body = objectBody(raw)
	return {
		user_id: stringField(body, 'user_id', 64),
		server_id: stringField(body, 'server_id', 64),
		keep_core: body.keep_core === true,
	}
}

function parseForgottenServer(raw: unknown): ForgottenServer {
	const body = objectBody(raw)
	return {
		server_id: stringField(body, 'server_id', 64),
		share_id: body.share_id == null ? null : stringField(body, 'share_id', 64),
	}
}

/**
 * App: the account's own Cores, then the ones it was invited to. Nobody else is told where a Core
 * is.
 */
async function listCores(request: Request, env: Env): Promise<ListedCore[]> {
	const user = await requireUser(request, env)
	const rows = await env.DB.prepare(
		'SELECT * FROM cores WHERE owner_id = ?1 OR id IN (SELECT core_id FROM core_access WHERE user_id = ?1) ORDER BY owner_id = ?1 DESC, created',
	)
		.bind(user.id)
		.all<CoreRow>()
	return rows.results.map((row) => publicCore(row, user.id))
}

/**
 * App: the owner takes a Core off the account. Its key stops working at once; the Core notices on
 * its next call and signs out on its own. Its server-first shares go with it: only that Core could
 * write them.
 */
async function removeCore(request: Request, env: Env, id: string) {
	const user = await requireUser(request, env)
	const deleted = await env.DB.prepare('DELETE FROM cores WHERE id = ? AND owner_id = ?')
		.bind(id, user.id)
		.run()
	if (!deleted.meta.changes) throw new ApiError(404, 'not_found', 'Core not found')
	const shares = (
		await env.DB.prepare(
			'DELETE FROM shared_instances WHERE core_id = ? AND owner_id = ? RETURNING id',
		)
			.bind(id, user.id)
			.all<{ id: string }>()
	).results
	for (const share of shares) await sweepFiles(env, share.id)
}

export const coreRoutes: Route[] = [
	route(cores.RequestSignInCode, parseNewSignInCode, async ({ request, env, body }) =>
		requestCode(request, env, await body()),
	),
	route(cores.PollSignIn, noBody, ({ request, env }) => pollCode(request, env)),
	route(cores.WaitForSignIn, noBody, ({ request, env }) => waitForApproval(request, env)),
	route(cores.ReportState, parseStateReport, ({ request, env, body }) =>
		reportState(request, env, body),
	),
	route(
		cores.ReportStorage,
		parseStorageReport,
		async ({ request, env, body }) => {
			const core = await requireListedCore(request, env)
			return reportStorage(env, core, await body())
		},
		MAX_REPORT_BODY,
	),
	route(cores.ReplaceAccess, parseAccessDirectory, ({ request, env, body }) =>
		replaceAccess(request, env, body),
	),
	route(cores.GrantAccess, parseAccessGrant, ({ request, env, body }) =>
		grantAccess(request, env, body),
	),
	route(cores.RevokeAccess, parseAccessRevocation, ({ request, env, body }) =>
		revokeAccess(request, env, body),
	),
	// A server was deleted or stopped following an instance: its players and links go. With
	// `share_id`, only while it still follows that share.
	route(cores.ForgetPlayers, parseForgottenServer, async ({ request, env, body }) => {
		const core = await requireListedCore(request, env)
		const { server_id: serverId, share_id: shareId } = await body()
		await forgetServer(env, core.id, serverId, shareId)
	}),
	route(cores.LookUpSignInCode, parseEnteredCode, ({ request, env, body }) =>
		lookupCode(request, env, body),
	),
	route(cores.ApproveSignInCode, parseEnteredCode, ({ request, env, body }) =>
		approveCode(request, env, body),
	),
	route(cores.ListCores, noBody, ({ request, env }) => listCores(request, env)),
	route(cores.IssueTicket, noBody, ({ request, env, params }) =>
		issueTicket(request, env, params.id),
	),
	route(cores.RemoveCore, noBody, ({ request, env, params }) =>
		removeCore(request, env, params.id),
	),
]
