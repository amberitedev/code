// Port of apps/labrinth/src/routes/internal/{flows,session}.rs, AGPL-3.0-only.
import zxcvbn from 'zxcvbn'
import {
	findUser,
	issueSession,
	publicUser,
	requireLocal,
	requireSession,
	requireUser,
	sessionResponse,
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
import type { Env, UserRow } from '../common/types'

type Flow = { id: string; kind: string; user_id: string; payload: string | null; expires: string }
const noContent = () => new Response(null, { status: 204 })

function validateUsername(username: string) {
	if (!/^[a-zA-Z0-9._-]{1,39}$/.test(username))
		invalid('Username must contain 1 to 39 letters, numbers, dots, underscores, or hyphens')
}
function validateEmail(email: string) {
	if (email.length > 254 || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email))
		invalid('Invalid email address!')
}
function validatePassword(password: string, username: string, email: string) {
	if (password.length < 8 || password.length > 256 || zxcvbn(password, [username, email]).score < 3)
		invalid('Specified password is too weak! Please improve its strength.')
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
	const flow = await createFlow(
		env,
		kind,
		user.id,
		kind === 'verify_email' ? user.email : null,
		1440,
	)
	await env.DB.prepare(
		'INSERT INTO email_outbox (id,recipient,kind,flow,created) VALUES (?,?,?,?,?)',
	)
		.bind(randomId(), user.email, kind, flow, new Date().toISOString())
		.run()
}
async function throttle(request: Request, env: Env) {
	const key = `auth:${request.headers.get('CF-Connecting-IP') ?? 'local'}`
	const now = Date.now()
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

export async function handleAccounts(request: Request, env: Env): Promise<Response | null> {
	const path = routePath(request),
		method = request.method
	if (['/_internal/globals', '/globals'].includes(path) && method === 'GET') {
		return json({
			captcha_enabled: env.LOCAL_DEV !== 'true',
			tax_compliance_thresholds: { 2025: 600, 2026: 2000 },
		})
	}
	if (path === '/_dev/session' && method === 'POST') {
		requireLocal(request, env)
		const username = stringField(await readJson(request), 'username', 39)
		validateUsername(username)
		let user = await findUser(env, username)
		if (user && !user.is_dev)
			throw new ApiError(403, 'invalid_credentials', 'Scenario login cannot access a real account')
		if (!user) {
			await env.DB.prepare(
				'INSERT INTO users (id,username,email,created,is_dev,email_verified,password_hash) VALUES (?,?,?,?,1,1,?)',
			)
				.bind(
					randomId(),
					username,
					`${username}@scenario.invalid`,
					new Date().toISOString(),
					await hashPassword(`Scenario-${username}-Local-only!`),
				)
				.run()
			user = await findUser(env, username)
		}
		if (!user) unauthorized()
		return json(await issueSession(request, env, user.id))
	}
	if (path === '/_dev/outbox' && method === 'GET') {
		requireLocal(request, env)
		return json(
			(await env.DB.prepare('SELECT * FROM email_outbox ORDER BY created DESC LIMIT 100').all())
				.results,
		)
	}
	if (['/auth/create', '/auth/create/validate'].includes(path) && method === 'POST') {
		const body = await readJson(request)
		const username = stringField(body, 'username', 39),
			password = stringField(body, 'password', 256),
			email = stringField(body, 'email', 254)
		validateUsername(username)
		validateEmail(email)
		validatePassword(password, username, email)
		if (await findUser(env, username))
			throw new ApiError(400, 'username_taken', 'Username is already taken on Modrinth.')
		if (
			await env.DB.prepare('SELECT id FROM users WHERE email = ? COLLATE NOCASE')
				.bind(email)
				.first()
		)
			throw new ApiError(
				400,
				'duplicate_email',
				"Email is already registered on Modrinth. Try 'Forgot password' to access your account.",
			)
		if (path.endsWith('/validate')) return noContent()
		await challenge(request, env)
		if (body.account_consent !== true)
			invalid('You must accept the terms and privacy policy to create an account')
		if (env.LOCAL_DEV !== 'true')
			throw new ApiError(503, 'mail_error', 'Email delivery is not configured')
		const id = randomId()
		try {
			await env.DB.prepare(
				'INSERT INTO users (id,username,email,password_hash,created) VALUES (?,?,?,?,?)',
			)
				.bind(id, username, email, await hashPassword(password), new Date().toISOString())
				.run()
		} catch (error) {
			if (await findUser(env, username))
				throw new ApiError(400, 'username_taken', 'Username is already taken on Modrinth.')
			if (
				await env.DB.prepare('SELECT id FROM users WHERE email = ? COLLATE NOCASE')
					.bind(email)
					.first()
			)
				throw new ApiError(400, 'duplicate_email', 'Email is already registered on Modrinth.')
			throw error
		}
		const user = await findUser(env, id)
		if (!user) unauthorized()
		await queueEmail(env, user, 'verify_email')
		return json(await issueSession(request, env, id))
	}
	if (path === '/auth/login' && method === 'POST') {
		const body = await readJson(request)
		await challenge(request, env)
		const username = stringField(body, 'username', 254),
			password = stringField(body, 'password', 256)
		const user = await env.DB.prepare(
			'SELECT * FROM users WHERE username = ? COLLATE NOCASE OR email = ? COLLATE NOCASE',
		)
			.bind(username, username)
			.first<UserRow>()
		if (!(await checkPassword(password, user?.password_hash ?? null)) || !user) unauthorized()
		if (user.totp_secret)
			return json({
				error: '2fa_required',
				description: '2FA is required to complete this operation.',
				flow: await createFlow(env, 'login_2fa', user.id),
			})
		return json(await issueSession(request, env, user.id))
	}
	if (path === '/auth/login/2fa' && method === 'POST') {
		await throttle(request, env)
		const body = await readJson(request),
			flow = await getFlow(env, stringField(body, 'flow'), 'login_2fa')
		const user = await findUser(env, flow.user_id)
		if (!user) unauthorized()
		await validateCode(env, user, stringField(body, 'code'))
		await consumeFlow(env, flow)
		return json(await issueSession(request, env, user.id))
	}
	if (path === '/session/refresh' && method === 'POST') {
		const session = await requireSession(request, env, true)
		return json(await issueSession(request, env, session.user_id, session))
	}
	if (path === '/session/list' && method === 'GET') {
		const current = await requireSession(request, env)
		const sessions = await env.DB.prepare(
			'SELECT * FROM sessions WHERE user_id = ? AND expires > ? ORDER BY created DESC',
		)
			.bind(current.user_id, new Date().toISOString())
			.all<SessionRow>()
		return json(sessions.results.map((row) => sessionResponse(row, null, row.id === current.id)))
	}
	if (path.startsWith('/session/') && method === 'DELETE') {
		const user = await requireUser(request, env)
		const id = decodeURIComponent(path.slice('/session/'.length))
		await env.DB.prepare('DELETE FROM sessions WHERE user_id = ? AND (id = ? OR token_hash = ?)')
			.bind(user.id, id, await digest(id))
			.run()
		return noContent()
	}
	if (path === '/auth/password/reset' && method === 'POST') {
		const body = await readJson(request)
		await challenge(request, env)
		const username = stringField(body, 'username', 254)
		const user = await env.DB.prepare(
			'SELECT * FROM users WHERE username = ? COLLATE NOCASE OR email = ? COLLATE NOCASE',
		)
			.bind(username, username)
			.first<UserRow>()
		if (user) await queueEmail(env, user, 'reset_password')
		return noContent()
	}
	if (path === '/auth/password' && method === 'PATCH') {
		await throttle(request, env)
		const body = await readJson(request)
		const flow =
			typeof body.flow === 'string' ? await getFlow(env, body.flow, 'reset_password') : null
		const user = flow ? await findUser(env, flow.user_id) : await requireUser(request, env)
		if (!user) unauthorized()
		if (!flow && !(await checkPassword(stringField(body, 'old_password', 256), user.password_hash)))
			unauthorized()
		if (typeof body.new_password !== 'string')
			invalid(
				'You must have another authentication method added to remove password authentication!',
			)
		validatePassword(body.new_password, user.username, user.email)
		if (flow) await consumeFlow(env, flow)
		await env.DB.prepare('UPDATE users SET password_hash = ? WHERE id = ?')
			.bind(await hashPassword(body.new_password), user.id)
			.run()
		await addNotification(env, user.id, { type: 'password_changed' })
		return noContent()
	}
	if (path === '/auth/2fa/get_secret' && method === 'POST') {
		const user = await requireUser(request, env)
		if (user.totp_secret) invalid('User already has 2FA enabled on their account!')
		const secret = totpSecret()
		return json({ secret, flow: await createFlow(env, 'initialize_2fa', user.id, secret) })
	}
	if (path === '/auth/2fa' && method === 'POST') {
		await throttle(request, env)
		const user = await requireUser(request, env),
			body = await readJson(request)
		const flow = await getFlow(env, stringField(body, 'flow'), 'initialize_2fa')
		if (flow.user_id !== user.id || user.totp_secret || !flow.payload) unauthorized()
		await validateCode(env, user, stringField(body, 'code'), flow.payload, false)
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
		return json({ backup_codes: codes })
	}
	if (path === '/auth/2fa' && method === 'DELETE') {
		await throttle(request, env)
		const user = await requireUser(request, env),
			body = await readJson(request)
		await validateCode(env, user, stringField(body, 'code'))
		await env.DB.batch([
			env.DB.prepare('UPDATE users SET totp_secret = NULL WHERE id = ?').bind(user.id),
			env.DB.prepare('DELETE FROM backup_codes WHERE user_id = ?').bind(user.id),
		])
		await addNotification(env, user.id, { type: 'two_factor_removed' })
		return noContent()
	}
	if (path === '/auth/email/verify' && method === 'POST') {
		const flow = await getFlow(env, stringField(await readJson(request), 'flow'), 'verify_email')
		await consumeFlow(env, flow)
		await env.DB.prepare(
			'UPDATE users SET email_verified = 1 WHERE id = ? AND email = ? COLLATE NOCASE',
		)
			.bind(flow.user_id, flow.payload)
			.run()
		return noContent()
	}
	if (path === '/auth/email/resend_verify' && method === 'POST') {
		await throttle(request, env)
		const user = await requireUser(request, env)
		if (!user.email_verified) await queueEmail(env, user, 'verify_email')
		return noContent()
	}
	if (path === '/auth/email' && method === 'PATCH') {
		const user = await requireUser(request, env),
			email = stringField(await readJson(request), 'email', 254)
		validateEmail(email)
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
		return noContent()
	}
	if (path === '/user' && method === 'GET')
		return json(publicUser(await requireUser(request, env), true))
	if (path === '/auth/passkey' && method === 'GET') {
		await requireUser(request, env)
		return json([])
	}
	if (path.startsWith('/auth/'))
		throw new ApiError(
			501,
			'unsupported_operation',
			'This authentication method has not been configured on this backend',
		)
	return null
}
