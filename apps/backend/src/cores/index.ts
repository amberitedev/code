import {
	type EnteredCode,
	type ForgottenServer,
	type ListedCore,
	type NewSignInCode,
	type SignInStatus,
	type StateReport,
	cores,
} from '@amberite/contracts/backend'

import { requireUser, tokenFrom } from '../common/auth'
import { isPrivateHost } from '../common/address'
import { digest, randomId } from '../common/crypto'
import { ApiError, invalid, objectBody, stringField, unauthorized } from '../common/http'
import { type Route, noBody, route } from '../common/router'
import type { Env } from '../common/types'
import { forgetServer, sweepFiles } from '../sharing'
import { introduce, parseDialer } from './link'
import { waitForApproval } from './sign-in'
import { MAX_REPORT_BODY, parseStorageReport, reportStorage } from './storage'
import { issueTicket } from './tickets'

// Core sign-in. A signed-out Core asks for a code and the key it will use, shows the code to its
// owner, and waits on a WebSocket that approval closes (`sign-in.ts`), polling only to read the
// answer. The owner enters the code in the App, which approves it with their session; from then
// on the key works and the Core is listed under that account. The App never talks to the Core
// during sign-in and the account session never leaves the App. After that the backend only
// keeps the account's list of Cores and their addresses, and introduces the owner's devices to
// them (`link.ts`). A Core reports its state on start and on
// change, never on a timer, and a report that changes nothing writes nothing: D1 writes are the
// free tier's scarcest resource. Only a Core's owner is told where it is or gets tickets for it.

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

function publicCore(row: CoreRow): ListedCore {
	return {
		id: row.id,
		name: row.name,
		address: row.address,
		cert_fingerprint: row.cert_fingerprint,
		public_address: row.public_address,
		version: row.version,
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
			'rate_limited',
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
	}
}

/**
 * Core, signed in: its whole state, sent once on start and again when any of it changes: where the
 * App reaches it, the certificate the App pins, and its version.
 * A report that changes nothing writes nothing. A Core removed from its account gets 410
 * `core_removed` and un-pairs.
 */
async function reportState(request: Request, env: Env, body: () => Promise<StateReport>) {
	const core = await requireListedCore(request, env)
	await writeState(env, core, await body())
}

/**
 * A Core as its own calls see it, with whether its owner's account is locked or may not use
 * sharing, so sharing needs no second read.
 */
type CallingCore = CoreRow & { owner_locked: number; owner_blacklisted: number }

/** The Core a request's key belongs to, or null for a key no Core has. */
async function findCore(request: Request, env: Env): Promise<CallingCore | null> {
	const key = tokenFrom(request)
	if (!key.startsWith('amc_')) unauthorized()
	return env.DB.prepare(
		'SELECT c.*, EXISTS(SELECT 1 FROM user_locks l WHERE l.user_id = c.owner_id) AS owner_locked, EXISTS(SELECT 1 FROM sharing_blacklist b WHERE b.user_id = c.owner_id) AS owner_blacklisted FROM cores c WHERE c.key_hash = ?',
	)
		.bind(await digest(key))
		.first<CallingCore>()
}

/**
 * The Core a request's key belongs to. A key that belonged to a Core that was since removed from
 * its account gets 410, so the Core un-pairs on that answer and on no other.
 */
export async function requireListedCore(request: Request, env: Env): Promise<CallingCore> {
	const core = await findCore(request, env)
	if (!core)
		throw new ApiError(
			410,
			'core_removed',
			'This Core was removed from its account. Pair it again.',
		)
	return core
}

/** Core, signed in: the WebSocket it is introduced to dialers on (`link.ts`). */
async function linkCore(request: Request, env: Env): Promise<Response> {
	if (request.headers.get('upgrade')?.toLowerCase() !== 'websocket')
		return new Response('WebSocket required', { status: 426 })
	const core = await requireListedCore(request, env)
	return env.CORE_LINK.getByName(core.id).fetch(request)
}

function parseForgottenServer(raw: unknown): ForgottenServer {
	const body = objectBody(raw)
	return {
		server_id: stringField(body, 'server_id', 64),
		share_id: body.share_id == null ? null : stringField(body, 'share_id', 64),
	}
}

/** App: the account's Cores. Nobody else is told where a Core is. */
async function listCores(request: Request, env: Env): Promise<ListedCore[]> {
	const user = await requireUser(request, env)
	const rows = await env.DB.prepare('SELECT * FROM cores WHERE owner_id = ? ORDER BY created')
		.bind(user.id)
		.all<CoreRow>()
	return rows.results.map(publicCore)
}

/**
 * App: the owner takes a Core off the account. Its key stops working at once; the Core notices on
 * its next call and signs out on its own.
 */
async function removeCore(request: Request, env: Env, id: string) {
	const user = await requireUser(request, env)
	if (!(await deleteCore(env, id, user.id))) throw new ApiError(404, 'not_found', 'Core not found')
}

/**
 * Deletes a Core's row, and its server-first shares with it: only that Core could write them.
 * False when the account has no such Core.
 */
async function deleteCore(env: Env, id: string, ownerId: string): Promise<boolean> {
	const deleted = await env.DB.prepare('DELETE FROM cores WHERE id = ? AND owner_id = ?')
		.bind(id, ownerId)
		.run()
	if (!deleted.meta.changes) return false
	try {
		const shares = (
			await env.DB.prepare(
				'DELETE FROM shared_instances WHERE core_id = ? AND owner_id = ? RETURNING id',
			)
				.bind(id, ownerId)
				.all<{ id: string }>()
		).results
		for (const share of shares) await sweepFiles(env, share.id)
	} finally {
		// Without it the Core learns on its next call to the backend.
		await env.CORE_LINK.getByName(id)
			.unlink()
			.catch((error: unknown) => console.warn('could not drop the link of a removed Core', error))
	}
	return true
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
	route(cores.LinkCore, noBody, ({ request, env }) => linkCore(request, env)),
	route(cores.Introduce, parseDialer, ({ request, env, params, body }) =>
		introduce(request, env, params.id, body),
	),
	route(cores.RemoveCore, noBody, ({ request, env, params }) =>
		removeCore(request, env, params.id),
	),
	// The Core's own CLI unpairs it. A Core already removed gets 410 `core_removed`.
	route(cores.LeaveAccount, noBody, async ({ request, env }) => {
		const core = await requireListedCore(request, env)
		await deleteCore(env, core.id, core.owner_id)
	}),
]
