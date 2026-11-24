import assert from 'node:assert/strict'
import test from 'node:test'

import { startBackend } from './worker.mjs'

test('friend refusals keep labrinth codes and text, avatars live in R2 under immutable URLs', async () => {
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

		// Labrinth's descriptions, which upstream's pages expect.
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

		// An avatar is an R2 object under a URL that changes with every upload, so it caches forever.
		const png = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10, 1, 2, 3])
		const avatarUrl = async (user = 'alice') =>
			JSON.parse(await call(`/v3/user/${user}`, { expected: 200 })).avatar_url
		const stored = async () =>
			(await backend.files.list({ prefix: 'avatars/' })).objects.map((object) => object.key)
		await call('/v3/user/alice/icon?ext=png', { method: 'PATCH', body: png })
		await call('/v3/user/alice/icon?ext=gif', { method: 'PATCH', body: png, expected: 400 })
		const first = await avatarUrl()
		const avatar = await backend.fetch(first)
		assert.equal(avatar.headers.get('content-type'), 'image/png')
		assert.equal(avatar.headers.get('cache-control'), 'public, max-age=31536000, immutable')
		assert.deepEqual(new Uint8Array(await avatar.arrayBuffer()), png)
		// A new upload replaces the object; removing the avatar and deleting the account drop it.
		await call('/v3/user/alice/icon?ext=png', { method: 'PATCH', body: png })
		const second = await avatarUrl()
		assert.notEqual(second, first)
		assert.equal((await backend.fetch(first)).status, 404)
		assert.deepEqual(await stored(), [`avatars/alice/${second.split('/').pop()}`])
		await call('/v3/user/alice/icon', { method: 'DELETE' })
		assert.equal(await avatarUrl(), null)
		assert.deepEqual(await stored(), [])
		await call('/v3/user/bob/icon?ext=png', { method: 'PATCH', body: png, user: 'bob' })
		assert.equal((await stored()).length, 1)
		await call('/v3/user/bob', { method: 'DELETE', user: 'bob' })
		assert.deepEqual(await stored(), [])

		// The App reaches the friends socket under labrinth's internal prefix.
		await call('/_internal/launcher_socket?code=mra_alice', { expected: 426 })
	} finally {
		await backend.dispose()
	}
})

/** Opens `user`'s friends socket; `next` waits for the next message that `match` accepts. */
async function connect(backend, user) {
	const response = await backend.fetch(
		`http://localhost/_internal/launcher_socket?code=mra_${user}`,
		{
			headers: { upgrade: 'websocket' },
		},
	)
	const socket = response.webSocket
	assert.ok(socket, `${user}'s socket did not open`)
	const queue = []
	const waiting = []
	socket.addEventListener('message', ({ data }) => {
		queue.push(JSON.parse(data))
		for (const wait of waiting.splice(0)) wait()
	})
	socket.accept()
	return {
		socket,
		async next(match) {
			for (;;) {
				const index = queue.findIndex(match)
				if (index >= 0) return queue.splice(index, 1)[0]
				await new Promise((resolve) => waiting.push(resolve))
			}
		},
	}
}

test('presence reaches the friends online, both ways, after each learns the other is', async () => {
	const backend = await startBackend('presence')
	try {
		for (const user of ['alice', 'bob']) await backend.account(user)
		await backend.db
			.prepare(
				"INSERT INTO friends (user_id,friend_id,accepted,created) VALUES ('alice','bob',1,'2026-01-01')",
			)
			.run()
		const alice = await connect(backend, 'alice')
		await alice.next((message) => message.type === 'friend_statuses')
		// Bob learns Alice is online from her answer; Alice learns of Bob from his message.
		const bob = await connect(backend, 'bob')
		const synced = await bob.next((message) => message.type === 'friend_statuses')
		assert.deepEqual(
			synced.statuses.map((status) => status.user_id),
			['alice'],
		)
		await alice.next(
			(message) => message.type === 'status_update' && message.status.user_id === 'bob',
		)
		const playing = (user) => (message) =>
			message.type === 'status_update' &&
			message.status.user_id === user &&
			message.status.profile_name === 'Survival'
		alice.socket.send(JSON.stringify({ type: 'status_update', profile_name: 'Survival' }))
		await bob.next(playing('alice'))
		bob.socket.send(JSON.stringify({ type: 'status_update', profile_name: 'Survival' }))
		await alice.next(playing('bob'))
		bob.socket.close()
		await alice.next((message) => message.type === 'user_offline' && message.id === 'bob')
		alice.socket.close()
	} finally {
		await backend.dispose()
	}
})
