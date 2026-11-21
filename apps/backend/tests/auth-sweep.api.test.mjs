import assert from 'node:assert/strict'
import test from 'node:test'

import { startBackend } from './worker.mjs'

test('expired sign-in flows and sessions are swept, and refreshable sessions stay', async () => {
	const backend = await startBackend('auth-sweep')
	try {
		await backend.account('friend')
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
		const response = await backend.fetch('http://local.test/auth/password', {
			method: 'PATCH',
			headers: { authorization: 'Bearer mra_friend', 'content-type': 'application/json' },
			body: JSON.stringify({ flow: 'invalid-flow' }),
		})
		assert.equal(response.status, 401)
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
	} finally {
		await backend.dispose()
	}
})
