import assert from 'node:assert/strict'
import { createHash, randomBytes } from 'node:crypto'
import test from 'node:test'
import { zstdCompressSync } from 'node:zlib'

import { startBackend } from './worker.mjs'

// Every request here is one the App sends from `packages/app-lib/src/api/instance/shared`, with the
// same method, path, body and headers, and every response is checked for the fields that code reads.
const MB = 1024 * 1024
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')
const sha512 = (bytes) => createHash('sha512').update(bytes).digest('hex')
const manifest = {
	modrinth_ids: ['AABBCCDD'],
	modpack_id: null,
	game_version: '1.21.1',
	loader: 'fabric',
	loader_version: '0.16.9',
}

/**
 * `send_external_file_batch`: a zstd-compressed multipart body whose first part is the JSON manifest
 * and whose `file` parts follow in manifest order. A file may carry a wrong `sha512` on purpose.
 */
function batch(files) {
	const boundary = `modrinth-${randomBytes(8).toString('hex')}`
	const part = (name, type, bytes) =>
		Buffer.concat([
			Buffer.from(
				`--${boundary}\r\nContent-Disposition: form-data; name="${name}"\r\nContent-Type: ${type}\r\n\r\n`,
			),
			bytes,
			Buffer.from('\r\n'),
		])
	const listed = files.map((file) => ({
		file_name: file.file_name,
		file_type: file.file_type,
		file_size: file.bytes.length,
		sha512: file.sha512 ?? sha512(file.bytes),
	}))
	const body = Buffer.concat([
		part('manifest', 'application/json', Buffer.from(JSON.stringify({ files: listed }))),
		...files.map((file) => part('file', 'application/octet-stream', file.bytes)),
		Buffer.from(`--${boundary}--\r\n`),
	])
	return {
		body: zstdCompressSync(body),
		headers: {
			'content-type': `multipart/form-data; boundary=${boundary}`,
			'content-encoding': 'zstd',
		},
	}
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
		// `upload_external_files`: POST one batch of files to the version.
		const upload = (id, version, external, user = 'owner') => {
			const { body, headers } = batch(external)
			return send(
				`http://local.test/v1/instances/${id}/versions/${version}/files`,
				'POST',
				user,
				body,
				headers,
			)
		}
		// `publish_current_content`: create the version, then upload the files it lists in one batch.
		const publish = async (id, external) => {
			const version = await ok(
				await call(`/instances/${id}/versions`, 'POST', {
					...manifest,
					external_files: external.map(({ file_name, file_type }) => ({ file_name, file_type })),
				}),
			)
			assert.deepEqual(
				version.external_files.map((file) => [file.file_name, file.file_type]),
				external.map((file) => [file.file_name, file.file_type]),
				'every file is requested for upload',
			)
			for (const file of version.external_files)
				assert.equal(new URL(file.url).origin, 'http://local.test', 'uploads stay on the API')
			if (!external.length) return { version }
			const response = await upload(id, version.version, external)
			if (!response.ok) return { version, failed: response.status, response }
			return { version, result: await response.json() }
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

		// Publish a mod that is not on Modrinth, a resource pack, and config files under their paths.
		const jar = { file_name: 'private-mod.jar', file_type: 'mod', bytes: randomBytes(300_000) }
		const pack = { file_name: 'look.zip', file_type: 'resourcepack', bytes: randomBytes(50_000) }
		const config = {
			file_name: 'config/private-mod/client.toml',
			file_type: 'config',
			bytes: Buffer.from('enabled = true\n'),
		}
		const options = { file_name: 'options.txt', file_type: 'config', bytes: Buffer.from('fov:0\n') }
		const first = await publish(id, [jar, pack, config, options])
		assert.equal(first.failed, undefined, `batch failed with ${first.failed}`)
		assert.equal(first.version.version, 1)
		assert.equal(first.version.ready, false)
		assert.equal(first.result.ready, true)
		assert.equal(await notifications(), 1, 'the friend is invited once the version is complete')
		assert.deepEqual(
			await stored(id),
			[jar, pack, config, options].map((file) => `${id}/${sha256(file.bytes)}`).sort(),
		)
		assert.equal(
			(await upload(id, 1, [jar])).status,
			409,
			'a complete version takes no more uploads',
		)

		// The friend sees the invite, accepts it, and downloads every file.
		assert.equal(
			(await ok(await call(`/instances/${id}`, 'GET', undefined, 'friend'))).name,
			'Proof pack',
		)
		assert.equal((await call(`/instances/${id}/versions`, 'GET', undefined, 'friend')).status, 401)
		assert.equal((await call(`/instances/${id}`, 'GET', undefined, 'stranger')).status, 401)
		await ok(await call(`/instances/${id}/invites/pending`, 'POST', undefined, 'friend'))
		assert.deepEqual(await ok(await call('/instances', 'GET', undefined, 'friend')), [id])
		const latest = await ok(await call(`/instances/${id}/versions`, 'GET', undefined, 'friend'))
		assert.equal((await call(`/instances/${id}/versions/not-a-number`)).status, 400)
		assert.deepEqual(
			{ ...latest, external_files: undefined },
			{ ...manifest, version: 1, ready: true, external_files: undefined },
		)
		for (const source of [jar, pack, config, options]) {
			const file = latest.external_files.find(
				(entry) => entry.file_type === source.file_type && entry.file_name === source.file_name,
			)
			assert.ok(file, `${source.file_name} is listed`)
			assert.equal(file.file_size, source.bytes.length)
			assert.equal(file.sha256, sha256(source.bytes))
			const download = await send(file.url, 'GET', 'friend')
			assert.equal(download.status, 200)
			assert.equal(Number(download.headers.get('content-length')), file.file_size)
			assert.equal(sha256(Buffer.from(await download.arrayBuffer())), file.sha256)
			assert.equal((await send(file.url, 'GET', 'stranger')).status, 401)
			assert.equal((await send(file.url, 'GET')).status, 401)
		}

		// A program is refused by content, naming the file the way the App reads it. The files before
		// it in the batch are kept, so a retried batch only has to carry the rest.
		const program = {
			file_name: 'mods/helper.jar',
			file_type: 'config',
			bytes: Buffer.concat([Buffer.from('MZ'), randomBytes(1000)]),
		}
		const refused = await publish(id, [config, program, jar])
		assert.equal(refused.failed, 400)
		assert.equal(refused.response.headers.get('x-file-not-allowed'), 'true')
		assert.ok((await refused.response.text()).endsWith(program.file_name))
		assert.equal((await ok(await call(`/instances/${id}/versions`))).version, 1)
		assert.equal((await upload(id, refused.version.version, [config, jar])).status, 200)
		assert.equal(
			(await ok(await call(`/instances/${id}/versions/${refused.version.version}`))).ready,
			false,
		)
		for (const [bad, status] of [
			[{ ...jar, sha512: sha512(pack.bytes) }, 400],
			[{ ...jar, file_name: '../escape.jar' }, 400],
			[pack, 400],
		]) {
			const version = await ok(
				await call(`/instances/${id}/versions`, 'POST', {
					...manifest,
					external_files: [{ file_name: jar.file_name, file_type: jar.file_type }],
				}),
			)
			assert.equal((await upload(id, version.version, [bad])).status, status)
		}
		const garbage = await send(
			`http://local.test/v1/instances/${id}/versions/${refused.version.version}/files`,
			'POST',
			'owner',
			Buffer.from('not zstd at all'),
			batch([]).headers,
		)
		assert.equal(garbage.status, 400, 'a body that is not zstd is the client’s mistake')

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

		// An unchanged file is stored once. The sixth complete version prunes the first and its files,
		// along with the uploads abandoned in between.
		const changed = { ...config, bytes: Buffer.from('enabled = false\n') }
		const pushed = []
		for (let push = 0; push < 4; push++) {
			const result = await publish(id, [jar, changed])
			assert.equal(result.failed, undefined)
			pushed.push(result.version.version)
		}
		assert.equal((await stored(id)).length, 5)
		assert.ok(await files.head(`${id}/${sha256(config.bytes)}`))
		const sixth = await publish(id, [jar, changed])
		const current = await ok(await call(`/instances/${id}/versions`))
		assert.equal(current.version, sixth.version.version)
		assert.equal(current.ready, true)
		assert.equal((await call(`/instances/${id}/versions/1`)).status, 404)
		assert.equal((await call(`/instances/${id}/versions/${refused.version.version}`)).status, 404)
		assert.equal(
			(await ok(await call(`/instances/${id}/versions/${pushed[0]}`))).version,
			pushed[0],
		)
		assert.equal(await files.head(`${id}/${sha256(config.bytes)}`), null)
		assert.deepEqual(
			await stored(id),
			[`${id}/${sha256(jar.bytes)}`, `${id}/${sha256(changed.bytes)}`].sort(),
		)
		assert.equal(
			(
				await send(
					latest.external_files.find((file) => file.file_name === config.file_name).url,
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
		assert.equal((await ok(await call(`/instances/${id}/versions`))).version, current.version)
		assert.equal((await ok(await call(`/instances/${id}/versions`))).ready, true)
		await publish(id, [jar])
		assert.equal((await ok(await call(`/instances/${id}/versions`))).version, current.version + 2)
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

		// A ready version loses its object: a download discovers it, and the next push repairs it.
		const repair = await ok(await call('/instances', 'POST', { name: 'Repair' }))
		await publish(repair.id, [jar])
		await publish(repair.id, [jar])
		const beforeLoss = await ok(await call(`/instances/${repair.id}/versions`))
		await files.delete(`${repair.id}/${sha256(jar.bytes)}`)
		const missing = await send(beforeLoss.external_files[0].url, 'GET', 'owner')
		assert.equal(missing.status, 404)
		assert.equal((await missing.json()).code, 'file_missing')
		for (const version of [1, 2]) {
			const broken = await ok(await call(`/instances/${repair.id}/versions/${version}`))
			assert.equal(broken.ready, false)
			assert.equal(broken.external_files[0].sha256, undefined)
			assert.ok(broken.external_files[0].url.endsWith(`/versions/${version}/files`))
		}
		const repaired = await publish(repair.id, [jar])
		assert.equal(repaired.result.ready, true)
		assert.equal((await ok(await call(`/instances/${repair.id}/versions`))).ready, true)
		await ok(await call(`/instances/${repair.id}`, 'DELETE'))
	} finally {
		await backend.dispose()
	}
})
