import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import test from 'node:test'

import { startBackend } from './worker.mjs'

test('Core re-invites replace notifications and deliver them even when access already exists', async () => {
	const backend = await startBackend('core-access-contract')
	let socket
	try {
		for (const user of ['owner', 'friend', 'stranger']) await backend.account(user)
		await backend.db
			.prepare(
				'INSERT INTO cores (id,owner_id,name,address,key_hash,created,last_seen) VALUES (?,?,?,?,?,?,?)',
			)
			.bind(
				'core',
				'owner',
				'Core',
				'http://localhost:1234',
				createHash('sha256').update('amc_test').digest('hex'),
				'2026-01-01',
				'2026-01-01',
			)
			.run()
		const call = (method, body, token = 'amc_test', path = '/core/access') =>
			backend.fetch(`http://local.test${path}`, {
				method,
				headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' },
				body: body === undefined ? undefined : JSON.stringify(body),
			})
		const invite = {
			server_id: 'server',
			server_name: 'Server',
			role: 'moderator',
			user: 'friend',
		}
		await backend.db
			.prepare('INSERT INTO auth_attempts (key,count,expires) VALUES (?,1,?)')
			.bind('expired-core', Date.now() - 1000)
			.run()
		assert.equal((await call('POST', invite, 'mra_owner')).status, 401)
		assert.equal((await call('GET', undefined, 'amc_test', '/cores')).status, 401)
		await backend.db
			.prepare(
				"INSERT INTO auth_flows (id,kind,user_id,expires) VALUES ('expired-flow','login_2fa','friend','2020-01-01')",
			)
			.run()
		await backend.db
			.prepare(
				"INSERT INTO sessions SELECT 'expired-session','expired-hash',user_id,created,last_login,'2020-01-01','2020-01-01',user_agent,ip FROM sessions WHERE id = 'friend'",
			)
			.run()
		await backend.db
			.prepare(
				"INSERT INTO sessions SELECT 'refreshable-session','refreshable-hash',user_id,created,last_login,'2020-01-01','2099-01-01',user_agent,ip FROM sessions WHERE id = 'friend'",
			)
			.run()
		assert.equal(
			(await call('PATCH', { flow: 'invalid-flow' }, 'mra_friend', '/auth/password')).status,
			401,
		)
		assert.equal(
			await backend.db.prepare("SELECT id FROM auth_flows WHERE id = 'expired-flow'").first(),
			null,
		)
		assert.equal(
			await backend.db.prepare("SELECT id FROM sessions WHERE id = 'expired-session'").first(),
			null,
		)
		assert.ok(
			await backend.db.prepare("SELECT id FROM sessions WHERE id = 'refreshable-session'").first(),
		)
		const connection = await backend.fetch(
			'http://local.test/_internal/launcher_socket?code=mra_friend',
			{
				headers: { upgrade: 'websocket' },
			},
		)
		assert.equal(connection.status, 101)
		socket = connection.webSocket
		socket.accept()
		const messages = []
		socket.addEventListener('message', (event) => messages.push(JSON.parse(event.data)))
		const seen = new Set()
		for (const keepCore of [null, true, false]) {
			if (keepCore !== null)
				assert.equal(
					(await call('DELETE', { user_id: 'friend', server_id: 'server', keep_core: keepCore }))
						.status,
					204,
				)
			assert.equal((await call('POST', invite)).status, 200)
			assert.equal(
				await backend.db
					.prepare("SELECT key FROM auth_attempts WHERE key = 'expired-core'")
					.first(),
				null,
			)
			const rows = (
				await backend.db.prepare("SELECT * FROM notifications WHERE user_id = 'friend'").all()
			).results
			assert.equal(rows.length, 1)
			assert.equal(JSON.parse(rows[0].body).type, 'server_invite')
			assert.ok(!seen.has(rows[0].id), 'every re-invite creates a fresh notification')
			seen.add(rows[0].id)
			// dispatchFetch completes after notify(), but the client message event can arrive later.
			await new Promise((resolve, reject) => {
				const timer = setTimeout(
					() => reject(new Error('invite was not delivered over the friends socket')),
					2000,
				)
				const check = () => {
					if (messages.some((message) => message.id === rows[0].id)) {
						clearTimeout(timer)
						socket.removeEventListener('message', check)
						resolve()
					}
				}
				socket.addEventListener('message', check)
				check()
			})
		}
	} finally {
		socket?.close()
		await backend.dispose()
	}
})
