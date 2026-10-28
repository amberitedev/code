import { digest, randomId } from './crypto'
import { ApiError, unauthorized } from './http'
import type { Env, UserRow } from './types'

export interface SessionRow {
	id: string
	token_hash: string
	user_id: string
	created: string
	last_login: string
	expires: string
	refresh_expires: string
	user_agent: string
	ip: string
}

export function tokenFrom(request: Request): string {
	return (request.headers.get('Authorization') ?? '').replace(/^Bearer /i, '')
}

export async function requireSession(
	request: Request,
	env: Env,
	refresh = false,
): Promise<SessionRow> {
	return authenticate<SessionRow>(
		request,
		env,
		'SELECT s.*, EXISTS(SELECT 1 FROM user_locks l WHERE l.user_id = s.user_id) AS locked FROM sessions s WHERE s.token_hash = ?',
		refresh,
	)
}

/**
 * The row `query` reads for a request's token: a session's, with its expiry and whether its
 * account is locked. Every authenticated request pays for this, so it is one statement.
 */
async function authenticate<Row extends { expires: string; refresh_expires: string }>(
	request: Request,
	env: Env,
	query: string,
	refresh = false,
): Promise<Row> {
	const token = tokenFrom(request)
	if (!token.startsWith('mra_')) unauthorized()
	const row = await env.DB.prepare(query)
		.bind(await digest(token))
		.first<Row & { locked: number }>()
	if (!row || (refresh ? row.refresh_expires : row.expires) <= new Date().toISOString())
		unauthorized()
	if (row.locked) accountLocked()
	return row
}

function accountLocked(): never {
	throw new ApiError(403, 'account_locked', 'Your account is locked and cannot perform this action')
}

export async function accountLock(env: Env, userId: string) {
	return env.DB.prepare('SELECT locked_by,reason,created FROM user_locks WHERE user_id = ?')
		.bind(userId)
		.first<{ locked_by: string; reason: string; created: string }>()
}

export async function requireUnlocked(env: Env, userId: string) {
	if (await accountLock(env, userId)) accountLocked()
}

export async function requireUser(request: Request, env: Env): Promise<UserRow> {
	return authenticate<UserRow & { expires: string; refresh_expires: string }>(
		request,
		env,
		'SELECT u.*, s.expires, s.refresh_expires, EXISTS(SELECT 1 FROM user_locks l WHERE l.user_id = u.id) AS locked FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = ?',
	)
}

export function publicUser(user: UserRow, full = false) {
	return {
		id: user.id,
		username: user.username,
		avatar_url: user.avatar_url,
		raw_avatar_url: user.avatar_url,
		bio: user.bio,
		created: user.created,
		role: user.role,
		badges: 0,
		campaigns: { pride_26: null },
		auth_providers: full ? [] : null,
		email: full ? user.email : null,
		email_verified: full ? Boolean(user.email_verified) : null,
		has_password: full ? Boolean(user.password_hash) : null,
		has_totp: full ? Boolean(user.totp_secret) : null,
		payout_data: null,
		stripe_customer_id: null,
		github_id: null,
		allow_friend_requests: full ? Boolean(user.allow_friend_requests) : null,
		eligibility_verified_at: full ? user.created : null,
	}
}

export function sessionResponse(row: SessionRow, session: string | null = null, current = false) {
	return {
		id: row.id,
		session,
		user_id: row.user_id,
		created: row.created,
		last_login: row.last_login,
		expires: row.expires,
		refresh_expires: row.refresh_expires,
		user_agent: row.user_agent,
		ip: row.ip,
		os: null,
		platform: null,
		city: null,
		country: null,
		current,
	}
}

export async function issueSession(
	request: Request,
	env: Env,
	userId: string,
	previous?: SessionRow,
) {
	await requireUnlocked(env, userId)
	const token = `mra_${randomId(60)}`
	const now = new Date().toISOString()
	const row: SessionRow = {
		id: randomId(),
		token_hash: await digest(token),
		user_id: userId,
		created: now,
		last_login: now,
		expires: new Date(Date.now() + 14 * 86400000).toISOString(),
		refresh_expires:
			previous?.refresh_expires ?? new Date(Date.now() + 60 * 86400000).toISOString(),
		user_agent: request.headers.get('User-Agent') ?? 'Unknown',
		ip: request.headers.get('CF-Connecting-IP') ?? '127.0.0.1',
	}
	const insert = env.DB.prepare(
		'INSERT INTO sessions (id,token_hash,user_id,created,last_login,expires,refresh_expires,user_agent,ip) VALUES (?,?,?,?,?,?,?,?,?)',
	).bind(
		row.id,
		row.token_hash,
		row.user_id,
		row.created,
		row.last_login,
		row.expires,
		row.refresh_expires,
		row.user_agent,
		row.ip,
	)
	if (previous) {
		// Replacing the old row in one statement makes rotation atomic and rejects replays.
		const claimed = await env.DB.prepare(
			'UPDATE sessions SET id = ?, token_hash = ?, created = ?, last_login = ?, expires = ?, user_agent = ?, ip = ? WHERE id = ? AND refresh_expires > ? RETURNING id',
		)
			.bind(
				row.id,
				row.token_hash,
				row.created,
				row.last_login,
				row.expires,
				row.user_agent,
				row.ip,
				previous.id,
				now,
			)
			.first()
		if (!claimed) unauthorized()
	} else await insert.run()
	return sessionResponse(row, token)
}

// Port of labrinth's issue_auth_sessions. The website asks for `app_session` when it signs the App
// in, so the App keeps a session of its own instead of sharing the browser's.
export async function issueAuthSessions(
	request: Request,
	env: Env,
	userId: string,
	appSession: unknown,
) {
	const session = await issueSession(request, env, userId)
	if (appSession !== true) return session
	return { ...session, app_session: (await issueSession(request, env, userId)).session }
}

export async function findUser(env: Env, id: string) {
	return env.DB.prepare('SELECT * FROM users WHERE id = ? OR username = ? COLLATE NOCASE')
		.bind(id, id)
		.first<UserRow>()
}

export async function areFriends(env: Env, first: string, second: string): Promise<boolean> {
	return Boolean(
		await env.DB.prepare(
			'SELECT 1 FROM friends WHERE accepted = 1 AND ((user_id = ? AND friend_id = ?) OR (user_id = ? AND friend_id = ?))',
		)
			.bind(first, second, second, first)
			.first(),
	)
}

export function requireLocal(request: Request, env: Env) {
	const hostname = new URL(request.url).hostname
	if (
		env.LOCAL_DEV !== 'true' ||
		!['localhost', '127.0.0.1', '[::1]'].includes(hostname) ||
		!env.LOCAL_DEV_SECRET ||
		request.headers.get('x-local-dev-secret') !== env.LOCAL_DEV_SECRET
	) {
		throw new ApiError(404, 'not_found', 'Not found')
	}
}
