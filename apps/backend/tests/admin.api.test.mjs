import assert from 'node:assert/strict'
import test from 'node:test'

import { startBackend } from './worker.mjs'

test('admin permissions, locked accounts, and forced recovery preserve upstream behavior', async () => {
	const backend = await startBackend('admin-contract', { LOCAL_DEV_SECRET: 'dev-secret' })
	try {
		const { db, session } = backend
		for (const [id, role] of [
			['admin', 'admin'],
			['moderator', 'moderator'],
			['member', 'developer'],
		]) {
			await db
				.prepare(
					'INSERT INTO users (id,username,email,created,role,password_hash) VALUES (?,?,?,?,?,?)',
				)
				.bind(id, id, `${id}@example.test`, '2026-01-01', role, 'old-password')
				.run()
			await session(id)
		}
		async function call(path, method = 'GET', body, user = 'admin', expected = 204) {
			const response = await backend.fetch(`http://localhost/v3${path}`, {
				method,
				headers: { authorization: `Bearer mra_${user}`, 'content-type': 'application/json' },
				...(body === undefined ? {} : { body: JSON.stringify(body) }),
			})
			const text = await response.text()
			assert.equal(response.status, expected, `${method} ${path}: ${text}`)
			return text ? JSON.parse(text) : null
		}
		const adminPath = '/_internal/admin/user/member'
		await call(`${adminPath}/lock`, 'PUT', { reason: 'proof' }, 'member', 401)
		await call(`${adminPath}/lock`, 'PUT', { reason: 'proof' }, 'moderator', 401)
		await call('/_internal/admin/user/moderator/lock', 'PUT', { reason: 'proof' }, 'admin', 401)
		await call('/_internal/admin/user/admin/password-reset', 'POST', {}, 'admin', 401)
		await call(`${adminPath}/lock`, 'PUT', { reason: 'proof' })
		assert.equal(await db.prepare("SELECT id FROM sessions WHERE user_id = 'member'").first(), null)
		await session('member')
		await call('/user', 'GET', undefined, 'member', 403)
		assert.equal((await call('/user/member', 'GET', undefined, 'admin', 200)).lock.reason, 'proof')
		assert.equal((await call('/user/member', 'GET', undefined, 'member', 200)).lock, undefined)
		await call(`${adminPath}/password-reset`, 'POST', { email: 'recovery@example.test' })
		const target = await db
			.prepare("SELECT email,password_hash FROM users WHERE id = 'member'")
			.first()
		assert.equal(target.password_hash, null)
		assert.equal(target.email, 'recovery@example.test')
		const flow = await db
			.prepare("SELECT flow FROM email_outbox WHERE recipient = 'recovery@example.test'")
			.first()
		const password = 'Oxygen-river-562!Quartz-bicycle'
		await call('/auth/password', 'PATCH', { flow: flow.flow, new_password: password })
		await call('/auth/password', 'PATCH', { flow: flow.flow, new_password: password }, 'admin', 401)
		await call('/auth/login', 'POST', { username: 'member', password }, 'member', 403)
		await call(`${adminPath}/lock`, 'DELETE')
		await call('/auth/login', 'POST', { username: 'member', password }, 'member', 200)
		await db.prepare("UPDATE users SET totp_secret = 'secret' WHERE id = 'member'").run()
		const challenge = await call(
			'/auth/login',
			'POST',
			{ username: 'member', password },
			'member',
			200,
		)
		assert.equal(challenge.error, '2fa_required')
		assert.equal(typeof challenge.flow, 'string')
		await call('/auth/passkey/start', 'POST', {}, 'member', 501)
		await db
			.prepare("INSERT INTO backup_codes (user_id,code_hash) VALUES ('member','backup')")
			.run()
		await call(`${adminPath}/2fa`, 'DELETE')
		assert.equal(
			(await db.prepare("SELECT totp_secret FROM users WHERE id = 'member'").first()).totp_secret,
			null,
		)
		assert.equal(
			await db.prepare("SELECT * FROM backup_codes WHERE user_id = 'member'").first(),
			null,
		)
		await session('member')
		await call(`${adminPath}/sessions`, 'DELETE')
		await call('/user', 'GET', undefined, 'member', 401)

		async function devSession(username, secret) {
			const response = await backend.fetch('http://localhost/_dev/session', {
				method: 'POST',
				headers: {
					'content-type': 'application/json',
					...(secret ? { 'x-local-dev-secret': secret } : {}),
				},
				body: JSON.stringify({ username }),
			})
			return { status: response.status, body: await response.json() }
		}
		assert.equal((await devSession('scenario', undefined)).status, 404)
		assert.equal((await devSession('scenario', 'wrong')).status, 404)
		assert.equal((await devSession('member', 'dev-secret')).status, 403)
		const scenario = await devSession('scenario', 'dev-secret')
		assert.equal(scenario.status, 200)
		const user = await backend.fetch('http://localhost/v3/user', {
			headers: { authorization: `Bearer ${scenario.body.session}` },
		})
		assert.equal((await user.json()).username, 'scenario')
	} finally {
		await backend.dispose()
	}
})
