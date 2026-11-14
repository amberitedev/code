// Port of apps/labrinth/src/routes/internal/{flows,session}.rs, AGPL-3.0-only.
import {
	accounts,
	type EmailUpdate,
	type EmailVerification,
	type ForcedPasswordReset,
	type LockReason,
	type LoginCredentials,
	type NewAccount,
	type PasswordReset,
	type PasswordUpdate,
	type TwoFactorCode,
	type TwoFactorLogin,
	type TwoFactorSetup,
} from '@amberite/contracts'
import zxcvbn from 'zxcvbn'

import {
	accountLock,
	findUser,
	issueAuthSessions,
	issueSession,
	publicUser,
	requireSession,
	requireUser,
	requireUnlocked,
	sessionDetails,
	type SessionRow,
} from '../common/auth'
import {
	checkPassword,
	digest,
	hashPassword,
	randomId,
	totpCounter,
	totpSecret,
} from '../common/crypto'
import { ApiError, invalid, objectBody, stringField, unauthorized } from '../common/http'
import { addNotification } from '../common/notifications'
import { type Route, noBody, route } from '../common/router'
import type { Env, UserRow } from '../common/types'
import { USER_ICON_BYTES } from '../social'

type Flow = { id: string; kind: string; user_id: string; payload: string | null; expires: string }

export function validateUsername(username: string) {
	if (!/^[a-zA-Z0-9._-]{1,39}$/.test(username))
		invalid('Username must contain 1 to 39 letters, numbers, dots, underscores, or hyphens')
}
function validateEmail(email: string) {
	if (email.length > 254 || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email))
		invalid('Invalid email address!')
}
function validatePassword(password: string, username: string, email: string, signup = false) {
	if (password.length < 8 || password.length > 256 || zxcvbn(password, [username, email]).score < 3)
		throw new ApiError(
			400,
			signup ? 'weak_password' : 'invalid_input',
			'Specified password is too weak! Please improve its strength.',
		)
}

async function createFlow(
	env: Env,
	kind: string,
	userId: string,
	payload: string | null = null,
	minutes = 30,
) {
	const id = randomId(48)
	await env.DB.prepare(
		'INSERT INTO auth_flows (id,kind,user_id,payload,expires) VALUES (?,?,?,?,?)',
	)
		.bind(id, kind, userId, payload, new Date(Date.now() + minutes * 60000).toISOString())
		.run()
	return id
}
async function getFlow(env: Env, id: string, kind: string) {
	const flow = await env.DB.prepare(
		'SELECT * FROM auth_flows WHERE id = ? AND kind = ? AND expires > ?',
	)
		.bind(id, kind, new Date().toISOString())
		.first<Flow>()
	if (!flow) unauthorized()
	return flow
}
async function consumeFlow(env: Env, flow: Flow) {
	if (
		!(await env.DB.prepare('DELETE FROM auth_flows WHERE id = ? RETURNING id')
			.bind(flow.id)
			.first())
	)
		unauthorized()
}
async function queueEmail(env: Env, user: UserRow, kind: 'verify_email' | 'reset_password') {
	// The local outbox is durable and readable only with the dev-runner secret.
	if (env.LOCAL_DEV !== 'true')
		throw new ApiError(503, 'mail_error', 'Email delivery is not configured')
	const flow = await createFlow(env, kind, user.id, user.email, 1440)
	await env.DB.prepare(
		'INSERT INTO email_outbox (id,recipient,kind,flow,created) VALUES (?,?,?,?,?)',
	)
		.bind(randomId(), user.email, kind, flow, new Date().toISOString())
		.run()
}
async function throttle(request: Request, env: Env) {
	const key = `auth:${request.headers.get('CF-Connecting-IP') ?? 'local'}`
	const now = Date.now()
	const nowIso = new Date(now).toISOString()
	await env.DB.batch([
		env.DB.prepare('DELETE FROM auth_attempts WHERE expires < ?').bind(now),
		env.DB.prepare('DELETE FROM auth_flows WHERE expires <= ?').bind(nowIso),
		// An expired access token may still be refreshable, so keep it until refresh expires.
		env.DB.prepare('DELETE FROM sessions WHERE refresh_expires <= ?').bind(nowIso),
	])
	const result = await env.DB.prepare(
		'INSERT INTO auth_attempts (key,count,expires) VALUES (?,1,?) ON CONFLICT(key) DO UPDATE SET count = CASE WHEN expires < ? THEN 1 ELSE count + 1 END, expires = CASE WHEN expires < ? THEN excluded.expires ELSE expires END RETURNING count',
	)
		.bind(key, now + 60000, now, now)
		.first<{ count: number }>()
	if (result && result.count > 30)
		throw new ApiError(
			429,
			'rate_limit',
			'Too many authentication attempts. Try again in a minute.',
		)
}
async function challenge(request: Request, env: Env) {
	await throttle(request, env)
	if (
		env.LOCAL_DEV === 'true' &&
		['127.0.0.1', 'localhost', '[::1]'].includes(new URL(request.url).hostname)
	)
		return
	throw new ApiError(503, 'configuration_error', 'Captcha verification is not configured')
}
async function validateCode(
	env: Env,
	user: UserRow,
	code: string,
	secret = user.totp_secret,
	backups = true,
) {
	if (!secret) unauthorized()
	const counter = await totpCounter(secret, code)
	if (counter !== null) {
		const result = await env.DB.prepare(
			'INSERT OR IGNORE INTO used_totp (user_id,counter) VALUES (?,?)',
		)
			.bind(user.id, counter)
			.run()
		if (result.meta.changes !== 1) unauthorized()
		await env.DB.prepare('DELETE FROM used_totp WHERE user_id = ? AND counter < ?')
			.bind(user.id, counter - 2)
			.run()
		return
	}
	if (
		backups &&
		(await env.DB.prepare(
			'DELETE FROM backup_codes WHERE user_id = ? AND code_hash = ? RETURNING code_hash',
		)
			.bind(user.id, await digest(code))
			.first())
	)
		return
	unauthorized()
}

function parseNewAccount(raw: unknown): NewAccount {
	const body = objectBody(raw)
	const username = stringField(body, 'username', 39),
		password = stringField(body, 'password', 256),
		email = stringField(body, 'email', 254)
	validateUsername(username)
	validateEmail(email)
	validatePassword(password, username, email, true)
	return {
		username,
		password,
		email,
		account_consent: body.account_consent === true,
		app_session: body.app_session === true,
	}
}

async function requireNewAccount(env: Env, { username, email }: NewAccount) {
	if (await findUser(env, username))
		throw new ApiError(400, 'username_taken', 'Username is already taken on Modrinth.')
	if (
		await env.DB.prepare('SELECT id FROM users WHERE email = ? COLLATE NOCASE').bind(email).first()
	)
		throw new ApiError(
			400,
			'duplicate_email',
			"Email is already registered on Modrinth. Try 'Forgot password' to access your account.",
		)
}

function parseLogin(raw: unknown): LoginCredentials {
	const body = objectBody(raw)
	return {
		username: stringField(body, 'username', 254),
		password: stringField(body, 'password', 256),
		app_session: body.app_session === true,
	}
}

function parseTwoFactorLogin(raw: unknown): TwoFactorLogin {
	const body = objectBody(raw)
	return {
		code: stringField(body, 'code'),
		flow: stringField(body, 'flow'),
		app_session: body.app_session === true,
	}
}

function parsePasswordReset(raw: unknown): PasswordReset {
	return { username: stringField(objectBody(raw), 'username', 254) }
}

function parsePasswordUpdate(raw: unknown): PasswordUpdate {
	const { flow, old_password: oldPassword, new_password: newPassword } = objectBody(raw)
	const text = (value: unknown) => (typeof value === 'string' ? value : undefined)
	return { flow: text(flow), old_password: text(oldPassword), new_password: text(newPassword) }
}

function parseTwoFactorSetup(raw: unknown): TwoFactorSetup {
	const body = objectBody(raw)
	return { code: stringField(body, 'code'), flow: stringField(body, 'flow') }
}

function parseTwoFactorCode(raw: unknown): TwoFactorCode {
	return { code: stringField(objectBody(raw), 'code') }
}

function parseEmailVerification(raw: unknown): EmailVerification {
	return { flow: stringField(objectBody(raw), 'flow') }
}

function parseEmailUpdate(raw: unknown): EmailUpdate {
	const email = stringField(objectBody(raw), 'email', 254)
	validateEmail(email)
	return { email }
}

function parseLockReason(raw: unknown): LockReason {
	const reason = stringField(objectBody(raw), 'reason', 65536).trim()
	if (!reason) invalid('Lock reason must not be empty')
	return { reason }
}

function parseForcedPasswordReset(raw: unknown): ForcedPasswordReset {
	const { email } = objectBody(raw)
	if (email === undefined || email === null) return {}
	if (typeof email !== 'string') invalid('Invalid email address')
	return { email: email.trim() || undefined }
}

/** The caller, if an admin, and the account they act on, which may be staff only when `staff`. */
async function adminTarget(request: Request, env: Env, id: string, staff = false) {
	const actor = await requireUser(request, env)
	if (actor.role !== 'admin')
		throw new ApiError(401, 'invalid_credentials', 'You must be an admin to manage user access')
	const target = await findUser(env, id)
	if (!target) throw new ApiError(404, 'not_found', 'User not found')
	if (!staff && (target.role === 'admin' || target.role === 'moderator'))
		throw new ApiError(
			401,
			'invalid_credentials',
			'Cannot lock or reset credentials of a staff account',
		)
	return { actor, target }
}

const MiB = 1024 * 1024

export const accountRoutes: Route[] = [
	route(accounts.GetGlobals, noBody, async ({ env }) => ({
		captcha_enabled: env.LOCAL_DEV !== 'true',
		tax_compliance_thresholds: { 2025: 600, 2026: 2000 },
		// Avatars are ours (`social/index.ts`); the rest are Modrinth's limits, for uploads that go
		// to Modrinth.
		max_user_icon_size: USER_ICON_BYTES,
		max_project_icon_size: 2 * MiB,
		max_organization_icon_size: MiB / 2,
		max_collection_icon_size: MiB / 2,
		max_oauth_client_icon_size: MiB / 2,
		max_gallery_image_size: 5 * MiB,
	})),
	route(accounts.ValidateNewAccount, parseNewAccount, async ({ env, body }) =>
		requireNewAccount(env, await body()),
	),
	route(accounts.CreateAccount, parseNewAccount, async ({ request, env, body }) => {
		const account = await body()
		await requireNewAccount(env, account)
		await challenge(request, env)
		if (!account.account_consent)
			invalid('You must accept the terms and privacy policy to create an account')
		if (env.LOCAL_DEV !== 'true')
			throw new ApiError(503, 'mail_error', 'Email delivery is not configured')
		const id = randomId()
		try {
			await env.DB.prepare(
				'INSERT INTO users (id,username,email,password_hash,created) VALUES (?,?,?,?,?)',
			)
				.bind(
					id,
					account.username,
					account.email,
					await hashPassword(account.password),
					new Date().toISOString(),
				)
				.run()
		} catch (error) {
			if (await findUser(env, account.username))
				throw new ApiError(400, 'username_taken', 'Username is already taken on Modrinth.')
			if (
				await env.DB.prepare('SELECT id FROM users WHERE email = ? COLLATE NOCASE')
					.bind(account.email)
					.first()
			)
				throw new ApiError(400, 'duplicate_email', 'Email is already registered on Modrinth.')
			throw error
		}
		const user = await findUser(env, id)
		if (!user) unauthorized()
		await queueEmail(env, user, 'verify_email')
		return issueAuthSessions(request, env, id, account.app_session)
	}),
	route(accounts.LogIn, parseLogin, async ({ request, env, body }) => {
		await challenge(request, env)
		const { username, password, app_session: appSession } = await body()
		const user = await env.DB.prepare(
			'SELECT * FROM users WHERE username = ? COLLATE NOCASE OR email = ? COLLATE NOCASE',
		)
			.bind(username, username)
			.first<UserRow>()
		if (!(await checkPassword(password, user?.password_hash ?? null)) || !user) unauthorized()
		await requireUnlocked(env, user.id)
		if (user.totp_secret)
			return {
				error: '2fa_required' as const,
				description: '2FA is required to complete this operation.',
				flow: await createFlow(env, 'login_2fa', user.id),
			}
		return issueAuthSessions(request, env, user.id, appSession)
	}),
	route(accounts.LogInTwoFactor, parseTwoFactorLogin, async ({ request, env, body }) => {
		await throttle(request, env)
		const login = await body()
		const flow = await getFlow(env, login.flow, 'login_2fa')
		const user = await findUser(env, flow.user_id)
		if (!user) unauthorized()
		await validateCode(env, user, login.code)
		await consumeFlow(env, flow)
		return issueAuthSessions(request, env, user.id, login.app_session)
	}),
	route(accounts.RefreshSession, noBody, async ({ request, env }) => {
		const session = await requireSession(request, env, true)
		return issueSession(request, env, session.user_id, session)
	}),
	route(accounts.ListSessions, noBody, async ({ request, env }) => {
		const current = await requireSession(request, env)
		const sessions = await env.DB.prepare(
			'SELECT * FROM sessions WHERE user_id = ? AND expires > ? ORDER BY created DESC',
		)
			.bind(current.user_id, new Date().toISOString())
			.all<SessionRow>()
		return sessions.results.map((row) => ({
			...sessionDetails(row, row.id === current.id),
			session: null,
		}))
	}),
	route(accounts.DeleteSession, noBody, async ({ request, env, params }) => {
		const user = await requireUser(request, env)
		const deleted = await env.DB.prepare(
			'DELETE FROM sessions WHERE user_id = ? AND (id = ? OR token_hash = ?)',
		)
			.bind(user.id, params.id, await digest(params.id))
			.run()
		if (deleted.meta.changes) await env.FRIENDS.getByName(user.id).revalidateSessions()
	}),
	route(accounts.RequestPasswordReset, parsePasswordReset, async ({ request, env, body }) => {
		await challenge(request, env)
		const { username } = await body()
		const user = await env.DB.prepare(
			'SELECT * FROM users WHERE username = ? COLLATE NOCASE OR email = ? COLLATE NOCASE',
		)
			.bind(username, username)
			.first<UserRow>()
		if (user && !(await accountLock(env, user.id))) await queueEmail(env, user, 'reset_password')
	}),
	route(accounts.ChangePassword, parsePasswordUpdate, async ({ request, env, body }) => {
		await throttle(request, env)
		// A reset email's flow stands in for the session, so the body is read first.
		const update = await body()
		const flow =
			update.flow === undefined
				? null
				: await env.DB.prepare(
						"SELECT * FROM auth_flows WHERE id = ? AND kind IN ('reset_password','forced_password_reset') AND expires > ?",
					)
						.bind(update.flow, new Date().toISOString())
						.first<Flow>()
		if (update.flow !== undefined && !flow) unauthorized()
		const user = flow ? await findUser(env, flow.user_id) : await requireUser(request, env)
		if (!user) unauthorized()
		if (flow && flow.payload !== user.email) unauthorized()
		if (flow?.kind !== 'forced_password_reset') await requireUnlocked(env, user.id)
		if (
			!flow &&
			!(await checkPassword(stringField(update, 'old_password', 256), user.password_hash))
		)
			unauthorized()
		if (update.new_password === undefined)
			invalid(
				'You must have another authentication method added to remove password authentication!',
			)
		validatePassword(update.new_password, user.username, user.email)
		if (flow) await consumeFlow(env, flow)
		await env.DB.prepare('UPDATE users SET password_hash = ? WHERE id = ?')
			.bind(await hashPassword(update.new_password), user.id)
			.run()
		await addNotification(env, user.id, { type: 'password_changed' })
	}),
	route(accounts.StartTwoFactorSetup, noBody, async ({ request, env }) => {
		const user = await requireUser(request, env)
		if (user.totp_secret) invalid('User already has 2FA enabled on their account!')
		const secret = totpSecret()
		return { secret, flow: await createFlow(env, 'initialize_2fa', user.id, secret) }
	}),
	route(accounts.EnableTwoFactor, parseTwoFactorSetup, async ({ request, env, body }) => {
		await throttle(request, env)
		const user = await requireUser(request, env)
		const setup = await body()
		const flow = await getFlow(env, setup.flow, 'initialize_2fa')
		if (flow.user_id !== user.id || user.totp_secret || !flow.payload) unauthorized()
		await validateCode(env, user, setup.code, flow.payload, false)
		await consumeFlow(env, flow)
		const codes = Array.from({ length: 6 }, () => randomId(11))
		await env.DB.batch([
			env.DB.prepare('UPDATE users SET totp_secret = ? WHERE id = ?').bind(flow.payload, user.id),
			env.DB.prepare('DELETE FROM backup_codes WHERE user_id = ?').bind(user.id),
			...(await Promise.all(
				codes.map(async (code) =>
					env.DB.prepare('INSERT INTO backup_codes (user_id,code_hash) VALUES (?,?)').bind(
						user.id,
						await digest(code),
					),
				),
			)),
		])
		await addNotification(env, user.id, { type: 'two_factor_enabled' })
		return { backup_codes: codes }
	}),
	route(accounts.DisableTwoFactor, parseTwoFactorCode, async ({ request, env, body }) => {
		await throttle(request, env)
		const user = await requireUser(request, env)
		await validateCode(env, user, (await body()).code)
		await env.DB.batch([
			env.DB.prepare('UPDATE users SET totp_secret = NULL WHERE id = ?').bind(user.id),
			env.DB.prepare('DELETE FROM backup_codes WHERE user_id = ?').bind(user.id),
		])
		await addNotification(env, user.id, { type: 'two_factor_removed' })
	}),
	route(accounts.VerifyEmail, parseEmailVerification, async ({ env, body }) => {
		const flow = await getFlow(env, (await body()).flow, 'verify_email')
		await consumeFlow(env, flow)
		await env.DB.prepare(
			'UPDATE users SET email_verified = 1 WHERE id = ? AND email = ? COLLATE NOCASE',
		)
			.bind(flow.user_id, flow.payload)
			.run()
	}),
	route(accounts.ResendVerifyEmail, noBody, async ({ request, env }) => {
		await throttle(request, env)
		const user = await requireUser(request, env)
		if (!user.email_verified) await queueEmail(env, user, 'verify_email')
	}),
	route(accounts.ChangeEmail, parseEmailUpdate, async ({ request, env, body }) => {
		const user = await requireUser(request, env)
		const { email } = await body()
		if (
			await env.DB.prepare('SELECT id FROM users WHERE email = ? COLLATE NOCASE AND id != ?')
				.bind(email, user.id)
				.first()
		)
			throw new ApiError(400, 'duplicate_email', 'Email is already registered on Modrinth.')
		await env.DB.prepare('UPDATE users SET email = ?, email_verified = 0 WHERE id = ?')
			.bind(email, user.id)
			.run()
		await queueEmail(env, { ...user, email }, 'verify_email')
	}),
	route(accounts.GetCurrentUser, noBody, async ({ request, env }) =>
		publicUser(await requireUser(request, env), true),
	),
	route(accounts.ListPasskeys, noBody, async ({ request, env }) => {
		await requireUser(request, env)
		return []
	}),
	route(accounts.UnlockUser, noBody, async ({ request, env, params }) => {
		const { target } = await adminTarget(request, env, params.id, true)
		const deleted = await env.DB.prepare('DELETE FROM user_locks WHERE user_id = ?')
			.bind(target.id)
			.run()
		if (!deleted.meta.changes) throw new ApiError(404, 'not_found', 'User is not locked')
	}),
	route(accounts.RevokeUserSessions, noBody, async ({ request, env, params }) => {
		const { target } = await adminTarget(request, env, params.id, true)
		// This backend only issues sessions; personal access tokens are not supported.
		await env.DB.prepare('DELETE FROM sessions WHERE user_id = ?').bind(target.id).run()
		await env.FRIENDS.getByName(target.id).revalidateSessions()
	}),
	route(accounts.LockUser, parseLockReason, async ({ request, env, params, body }) => {
		const { actor, target } = await adminTarget(request, env, params.id)
		const { reason } = await body()
		await env.DB.batch([
			env.DB.prepare(
				'INSERT INTO user_locks (user_id,locked_by,reason,created) VALUES (?,?,?,?) ON CONFLICT(user_id) DO UPDATE SET locked_by=excluded.locked_by,reason=excluded.reason,created=excluded.created',
			).bind(target.id, actor.id, reason, new Date().toISOString()),
			env.DB.prepare('DELETE FROM sessions WHERE user_id = ?').bind(target.id),
		])
		await env.FRIENDS.getByName(target.id).revalidateSessions()
	}),
	route(
		accounts.ForcePasswordReset,
		parseForcedPasswordReset,
		async ({ request, env, params, body }) => {
			const { target } = await adminTarget(request, env, params.id)
			const email = (await body()).email ?? target.email
			validateEmail(email)
			if (
				await env.DB.prepare('SELECT id FROM users WHERE email = ? COLLATE NOCASE AND id != ?')
					.bind(email, target.id)
					.first()
			)
				invalid('Email address is already registered to another account')
			if (env.LOCAL_DEV !== 'true')
				throw new ApiError(503, 'mail_error', 'Email delivery is not configured')
			const flow = randomId(48),
				now = new Date().toISOString()
			// Commit the credential reset and recovery email together so delivery failures cannot strand an account.
			await env.DB.batch([
				env.DB.prepare(
					'UPDATE users SET password_hash = NULL, email_verified = email_verified AND LOWER(email) = LOWER(?), email = ? WHERE id = ?',
				).bind(email, email, target.id),
				env.DB.prepare('DELETE FROM sessions WHERE user_id = ?').bind(target.id),
				env.DB.prepare(
					'INSERT INTO auth_flows (id,kind,user_id,payload,expires) VALUES (?,?,?,?,?)',
				).bind(
					flow,
					'forced_password_reset',
					target.id,
					email,
					new Date(Date.now() + 86400000).toISOString(),
				),
				env.DB.prepare(
					'INSERT INTO email_outbox (id,recipient,kind,flow,created) VALUES (?,?,?,?,?)',
				).bind(randomId(), email, 'reset_password', flow, now),
			])
			await env.FRIENDS.getByName(target.id).revalidateSessions()
		},
	),
	route(accounts.ResetUserTwoFactor, noBody, async ({ request, env, params }) => {
		const { target } = await adminTarget(request, env, params.id)
		if (!target.totp_secret) invalid('User does not have 2FA enabled')
		await env.DB.batch([
			env.DB.prepare('UPDATE users SET totp_secret = NULL WHERE id = ?').bind(target.id),
			env.DB.prepare('DELETE FROM backup_codes WHERE user_id = ?').bind(target.id),
			env.DB.prepare('DELETE FROM used_totp WHERE user_id = ?').bind(target.id),
			env.DB.prepare('INSERT INTO notifications (id,user_id,body,created) VALUES (?,?,?,?)').bind(
				randomId(),
				target.id,
				JSON.stringify({ type: 'two_factor_removed' }),
				new Date().toISOString(),
			),
		])
	}),
]
