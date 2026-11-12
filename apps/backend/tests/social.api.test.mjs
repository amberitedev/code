import assert from 'node:assert/strict'
import test from 'node:test'

import { startBackend } from './worker.mjs'

test('friend refusals keep labrinth codes and text, avatars round-trip as raw bytes', async () => {
	const backend = await startBackend('social-contract')
	try {
		await backend.account('alice')
		await backend.account('bob')
		async function call(path, { method = 'GET', user = 'alice', body, expected = 204 } = {}) {
			const response = await backend.fetch(`http://localhost${path}`, {
				method,
				headers: { authorization: `Bearer mra_${user}` },
				body,
			})
			const text = await response.text()
			assert.equal(response.status, expected, `${method} ${path}: ${text}`)
			return text
		}
		const refusal = async (path, user) =>
			JSON.parse(await call(path, { method: 'POST', user, expected: 400 }))

		// Upstream's access page (packages/ui) matches these descriptions.
		assert.deepEqual(await refusal('/v3/friend/alice', 'alice'), {
			error: 'cannot_friend_self',
			description: 'You cannot add yourself as a friend!',
		})
		await call('/v3/friend/bob', { method: 'POST' })
		assert.deepEqual(await refusal('/v3/friend/bob', 'alice'), {
			error: 'own_friend_request',
			description: 'You cannot accept your own friend request!',
		})
		await call('/v3/friend/alice', { method: 'POST', user: 'bob' })
		assert.deepEqual(await refusal('/v2/friend/alice', 'bob'), {
			error: 'already_friends',
			description: 'You are already friends with this user!',
		})

		const png = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10, 1, 2, 3])
		await call('/v3/user/alice/icon?ext=png', { method: 'PATCH', body: png })
		await call('/v3/user/alice/icon?ext=gif', { method: 'PATCH', body: png, expected: 400 })
		const { avatar_url: avatarUrl } = JSON.parse(await call('/v3/user/alice', { expected: 200 }))
		const avatar = await backend.fetch(avatarUrl)
		assert.equal(avatar.headers.get('content-type'), 'image/png')
		assert.deepEqual(new Uint8Array(await avatar.arrayBuffer()), png)

		// The App reaches the friends socket under labrinth's internal prefix.
		await call('/_internal/launcher_socket?code=mra_alice', { expected: 426 })
	} finally {
		await backend.dispose()
	}
})
