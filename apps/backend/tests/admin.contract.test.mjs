import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import test from 'node:test'

const require = createRequire(import.meta.url)
const wranglerRequire = createRequire(require.resolve('wrangler'))
const { Miniflare } = wranglerRequire('miniflare')
const { build } = wranglerRequire('esbuild')

test('admin permissions, locked accounts, and forced recovery preserve upstream behavior', async () => {
	const bundle = await build({
		entryPoints: [fileURLToPath(new URL('../src/index.ts', import.meta.url))],
		bundle: true,
		write: false,
		format: 'esm',
		platform: 'browser',
		target: 'es2022',
		external: ['cloudflare:workers'],
	})
	const runtime = new Miniflare({
		modules: true,
		script: bundle.outputFiles[0].text,
		compatibilityDate: '2026-07-22',
		compatibilityFlags: ['nodejs_compat'],
		d1Databases: { DB: 'admin-contract' },
		durableObjects: { FRIENDS: { className: 'FriendsHub', useSQLite: true } },
		bindings: { LOCAL_DEV: 'true' },
	})
	try {
		const db = await runtime.getD1Database('DB')
		for (const file of [
			'0001_accounts.sql',
			'0002_sharing.sql',
			'0003_sharing_idempotency.sql',
			'0004_account_locks.sql',
		]) {
			const sql = await readFile(new URL(`../migrations/${file}`, import.meta.url), 'utf8')
			for (const statement of sql
				.split(';')
				.map((value) => value.trim())
				.filter(Boolean))
				await db.prepare(statement).run()
		}
		async function session(user) {
			await db
				.prepare(
					'INSERT INTO sessions (id,token_hash,user_id,created,last_login,expires,refresh_expires,user_agent,ip) VALUES (?,?,?,?,?,?,?,?,?)',
				)
				.bind(
					user,
					createHash('sha256').update(`mra_${user}`).digest('hex'),
					user,
					'2026-01-01',
					'2026-01-01',
					'2099-01-01',
					'2099-01-01',
					'test',
					'127.0.0.1',
				)
				.run()
		}
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
			const response = await runtime.dispatchFetch(`http://localhost/v3${path}`, {
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
	} finally {
		await runtime.dispose()
	}
})
