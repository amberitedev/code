import assert from 'node:assert/strict'
import { createHash, randomBytes } from 'node:crypto'
import test from 'node:test'

import { startBackend } from './worker.mjs'

// Every request here is one the App sends from `packages/app-lib/src/api/instance/shared`, with the
// same method, path, body and headers, and every response is checked for the fields that code reads.
const MB = 1024 * 1024
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')
const manifest = {
	modrinth_ids: ['AABBCCDD'],
	modpack_id: null,
	game_version: '1.21.1',
	loader: 'fabric',
	loader_version: '0.16.9',
}

test('sharing API as the App uses it, with files in R2', async () => {
	const backend = await startBackend('sharing-contract')
	try {
		const { db, files } = backend
		for (const user of ['owner', 'friend', 'stranger']) await backend.account(user)
		await db
			.prepare('INSERT INTO friends (user_id,friend_id,accepted,created) VALUES (?,?,1,?)')
			.bind('owner', 'friend', new Date().toISOString())
			.run()
		const send = (url, method, user, body, headers = {}) =>
			backend.fetch(url, {
				method,
				headers: { ...(user ? { authorization: `Bearer mra_${user}` } : {}), ...headers },
				body,
			})
		const call = (path, method = 'GET', body, user = 'owner') =>
			send(
				`http://local.test/v1${path}`,
				method,
				user,
				body && JSON.stringify(body),
				body ? { 'content-type': 'application/json' } : {},
			)
		const ok = async (response) => {
			assert.ok(response.ok, `expected success, got ${response.status}`)
			return response.status === 204 ? null : response.json()
		}
		// `upload_external_files`: PUT the bytes to the URL the version returned.
		const put = (url, bytes, user = 'owner') =>
			send(url, 'PUT', user, bytes, {
				'content-type': 'application/octet-stream',
				'x-file-sha512': createHash('sha512').update(bytes).digest('hex'),
			})
		// `publish_current_content`: create the version, then upload each external file in order.
		const publish = async (id, external) => {
			const version = await ok(
				await call(`/instances/${id}/versions`, 'POST', {
					...manifest,
					external_files: external.map(({ file_name, file_type }) => ({ file_name, file_type })),
				}),
			)
			assert.equal(version.external_files.length, external.length)
			for (const upload of version.external_files) {
				const source = external.find(
					(file) => file.file_name === upload.file_name && file.file_type === upload.file_type,
				)
				assert.equal(new URL(upload.url).origin, 'http://local.test', 'uploads stay on the API')
				const response = await put(upload.url, source.bytes)
				if (!response.ok) return { version, failed: response.status }
			}
			return { version }
		}
		const stored = async (id) =>
			(await files.list({ prefix: `${id}/` })).objects.map((object) => object.key).sort()

		// Share an instance: create it, name it, give it an icon, invite a friend.
		const { id } = await ok(await call('/instances', 'POST', { name: 'Proof' }))
		assert.equal(typeof id, 'string')
		await ok(await call(`/instances/${id}`, 'PATCH', { name: 'Proof pack' }))
		const icon = randomBytes(64)
		await ok(
			await send(`http://local.test/v1/instances/${id}/icon`, 'PUT', 'owner', icon, {
				'content-type': 'application/octet-stream',
			}),
		)
		const info = await ok(await call(`/instances/${id}`))
		assert.equal(info.name, 'Proof pack')
		assert.equal(info.quarantine, false)
		assert.deepEqual(Buffer.from(await (await send(info.icon, 'GET')).arrayBuffer()), icon)
		await ok(await call(`/instances/${id}/icon`, 'DELETE'))
		assert.equal((await ok(await call(`/instances/${id}`))).icon, null)
		await ok(await call(`/instances/${id}/users`, 'POST', { user_ids: ['friend'] }))
		const notifications = async () =>
			(await db.prepare('SELECT COUNT(*) AS count FROM notifications').first()).count
		assert.equal(await notifications(), 0, 'no invitation before there is something to install')

		// Publish a version with a config bundle and a file that is not on Modrinth.
		const jar = { file_name: 'private-mod.jar', file_type: 'mod', bytes: randomBytes(300_000) }
		const bundle = { file_name: 'config.zip', file_type: 'configs', bytes: randomBytes(20_000) }
		const first = await publish(id, [jar, bundle])
		assert.equal(first.failed, undefined)
		assert.equal(first.version.version, 1)
		assert.equal(first.version.ready, false)
		assert.equal((await put(first.version.external_files[0].url, jar.bytes)).status, 409)
		assert.equal(await notifications(), 1, 'the friend is invited once the version is complete')
		assert.deepEqual(
			await stored(id),
			[`${id}/${sha256(jar.bytes)}`, `${id}/${sha256(bundle.bytes)}`].sort(),
		)

		// The friend sees the invite, accepts it, and downloads both files.
		assert.equal(
			(await ok(await call(`/instances/${id}`, 'GET', undefined, 'friend'))).name,
			'Proof pack',
		)
		assert.equal((await call(`/instances/${id}/versions`, 'GET', undefined, 'friend')).status, 401)
		assert.equal((await call(`/instances/${id}`, 'GET', undefined, 'stranger')).status, 401)
		await ok(await call(`/instances/${id}/invites/pending`, 'POST', undefined, 'friend'))
		assert.deepEqual(await ok(await call('/instances', 'GET', undefined, 'friend')), [id])
		const latest = await ok(await call(`/instances/${id}/versions`, 'GET', undefined, 'friend'))
		assert.deepEqual(
			{ ...latest, external_files: undefined },
			{ ...manifest, version: 1, ready: true, external_files: undefined },
		)
		for (const source of [jar, bundle]) {
			const file = latest.external_files.find((entry) => entry.file_type === source.file_type)
			assert.equal(file.file_name, source.file_name)
			assert.equal(file.file_size, source.bytes.length)
			assert.equal(file.sha256, sha256(source.bytes))
			const download = await send(file.url, 'GET', 'friend')
			assert.equal(download.status, 200)
			assert.equal(Number(download.headers.get('content-length')), file.file_size)
			assert.equal(sha256(Buffer.from(await download.arrayBuffer())), file.sha256)
			assert.equal((await send(file.url, 'GET', 'stranger')).status, 401)
			assert.equal((await send(file.url, 'GET')).status, 401)
		}

		// Members list and invite links.
		const members = await ok(await call(`/instances/${id}/users`))
		assert.equal(members.tokens, 0)
		assert.deepEqual(members.users.map((user) => [user.id, user.join_type]).sort(), [
			['friend', 'invite'],
			['owner', 'owner'],
		])
		for (const user of members.users) {
			assert.ok(!Number.isNaN(Date.parse(user.joined_at)))
			assert.equal(user.last_played, null)
		}
		const link = await ok(
			await call(`/instances/${id}/invites`, 'POST', { max_age: 3600, max_uses: 2 }),
		)
		const [listed] = await ok(await call(`/instances/${id}/invites`))
		assert.deepEqual(
			{ ...listed, expiration: undefined },
			{ id: link.id, max_uses: 2, uses: 0, expiration: undefined },
		)
		assert.ok(Date.parse(listed.expiration) > Date.now())
		assert.equal((await ok(await call(`/instances/${id}/users`))).tokens, 1)
		const preview = await ok(await send(`http://local.test/v1/invites/${link.id}`, 'GET'))
		assert.equal(preview.instance_id, id)
		assert.equal(preview.instance_name, 'Proof pack')
		assert.equal(preview.instance_icon, null)
		assert.deepEqual(
			preview.managers.map(({ type, id }) => ({ type, id })),
			[{ type: 'user', id: 'owner' }],
		)
		await ok(await call(`/instances/${id}/invites/${link.id}`, 'POST', undefined, 'stranger'))
		await ok(await call(`/instances/${id}/invites/${link.id}`, 'POST', undefined, 'stranger'))
		assert.equal((await ok(await call(`/instances/${id}/invites`)))[0].uses, 1)
		await ok(await call(`/instances/${id}/versions`, 'GET', undefined, 'stranger'))
		await ok(await call(`/instances/${id}/invites/${link.id}`, 'DELETE'))
		assert.equal((await send(`http://local.test/v1/invites/${link.id}`, 'GET')).status, 404)

		// Leaving and removal revoke access to versions and files alike.
		await ok(await call(`/instances/${id}/users`, 'DELETE', { user_ids: ['stranger'] }, 'stranger'))
		assert.equal(
			(await call(`/instances/${id}/users`, 'DELETE', { user_ids: ['owner'] }, 'friend')).status,
			403,
		)
		await ok(await call(`/instances/${id}/users`, 'DELETE', { user_ids: ['friend'] }))
		assert.equal((await call(`/instances/${id}/versions`, 'GET', undefined, 'friend')).status, 401)
		assert.equal((await send(latest.external_files[0].url, 'GET', 'friend')).status, 401)
		await ok(await call(`/instances/${id}/users`, 'POST', { user_ids: ['friend'] }))
		await ok(await call(`/instances/${id}/invites/pending`, 'DELETE', undefined, 'friend'))
		assert.equal(
			(await call(`/instances/${id}/invites/pending`, 'POST', undefined, 'friend')).status,
			404,
		)

		// An unchanged file is stored once. The sixth version prunes the first and its file.
		const changed = { ...bundle, bytes: randomBytes(20_000) }
		for (let version = 2; version <= 5; version++) {
			const result = await publish(id, [jar, changed])
			assert.equal(result.failed, undefined)
			assert.equal(result.version.version, version)
		}
		assert.equal((await stored(id)).length, 3)
		assert.ok(await files.head(`${id}/${sha256(bundle.bytes)}`))
		await publish(id, [jar, changed])
		const current = await ok(await call(`/instances/${id}/versions`))
		assert.equal(current.version, 6)
		assert.equal(current.ready, true)
		assert.equal((await call(`/instances/${id}/versions/1`)).status, 404)
		assert.equal((await ok(await call(`/instances/${id}/versions/2`))).version, 2)
		assert.equal(await files.head(`${id}/${sha256(bundle.bytes)}`), null)
		assert.deepEqual(
			await stored(id),
			[`${id}/${sha256(jar.bytes)}`, `${id}/${sha256(changed.bytes)}`].sort(),
		)
		assert.equal(
			(
				await send(
					latest.external_files.find((file) => file.file_type === 'configs').url,
					'GET',
					'owner',
				)
			).status,
			404,
		)

		// Over-limit files fail their upload, and the last complete version stays current.
		const tooLarge = {
			file_name: 'huge.jar',
			file_type: 'mod',
			bytes: Buffer.alloc(25 * MB + 1, 1),
		}
		assert.equal((await publish(id, [tooLarge])).failed, 413)
		const largeBundle = { ...bundle, bytes: Buffer.alloc(5 * MB + 1, 2) }
		assert.equal((await publish(id, [jar, largeBundle])).failed, 413)
		assert.equal(
			(
				await call(`/instances/${id}/versions`, 'POST', {
					...manifest,
					external_files: Array.from({ length: 21 }, (_, index) => ({
						file_name: `${index}.jar`,
						file_type: 'mod',
					})),
				})
			).status,
			413,
		)
		assert.equal((await ok(await call(`/instances/${id}/versions`))).version, 6)
		assert.equal((await ok(await call(`/instances/${id}/versions`))).ready, true)
		await publish(id, [jar])
		assert.equal((await ok(await call(`/instances/${id}/versions`))).version, 9)
		assert.deepEqual(
			(
				await db
					.prepare('SELECT version FROM shared_versions WHERE instance_id = ? AND ready = 0')
					.bind(id)
					.all()
			).results,
			[],
			'abandoned uploads are dropped by the next complete version',
		)

		// 100 MB per instance: four 25 MB files fit, one more byte does not.
		const full = await ok(await call('/instances', 'POST', { name: 'Full' }))
		const big = Array.from({ length: 4 }, (_, index) => ({
			file_name: `${index}.jar`,
			file_type: 'mod',
			bytes: Buffer.alloc(25 * MB, index + 1),
		}))
		assert.equal((await publish(full.id, big)).failed, undefined)
		assert.equal(
			(await publish(full.id, big)).failed,
			undefined,
			'unchanged files take no more room',
		)
		const extra = { file_name: 'extra.jar', file_type: 'mod', bytes: Buffer.from('x') }
		assert.equal((await publish(full.id, [...big, extra])).failed, 413)
		assert.equal((await stored(full.id)).length, 4)

		// Deleting the shared instance removes its files.
		await ok(await call(`/instances/${full.id}`, 'DELETE'))
		assert.equal((await call(`/instances/${full.id}`)).status, 404)
		assert.deepEqual(await stored(full.id), [])
		assert.equal((await stored(id)).length, 2, 'other instances keep their files')
	} finally {
		await backend.dispose()
	}
})
