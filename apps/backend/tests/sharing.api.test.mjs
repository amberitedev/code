import assert from 'node:assert/strict'
import { createHash, generateKeyPairSync, randomBytes, sign, verify } from 'node:crypto'
import test from 'node:test'

import { startBackend } from './worker.mjs'

// Every request here is one the App sends from `packages/engine/src/api/instance/shared` or a
// Core sends from `apps/core/src/storage.rs`, with the same method, path and body, and
// every response is checked for the fields that code reads. Files never reach the backend: the
// App sends a manifest of encrypted objects and the Core reports which versions it holds.

const GB = 1024 ** 3
const EMPTY_ROOT = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
const { privateKey, publicKey } = generateKeyPairSync('ed25519')
const kid = 'test-key'
const TICKET_KEY = JSON.stringify({ ...privateKey.export({ format: 'jwk' }), kid })
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')
const manifest = {
	modrinth_ids: ['AABBCCDD'],
	modpack_id: null,
	game_version: '1.21.1',
	loader: 'fabric',
	loader_version: '0.16.9',
}

/** The claims of a ticket, and whether the backend's key signed it. */
function decode(ticket) {
	const [header, claims, signature] = ticket.split('.')
	const json = (part) => JSON.parse(Buffer.from(part, 'base64url').toString())
	return {
		claims: json(claims),
		valid: verify(
			null,
			Buffer.from(`${header}.${claims}`),
			publicKey,
			Buffer.from(signature, 'base64url'),
		),
	}
}

/** A file as the App's push describes it, with `size` plain bytes behind a random object name. */
function file(file_name, file_type, size, key = file_type === 'config') {
	const object = sha256(randomBytes(32))
	return {
		file_name,
		file_type,
		file_size: size,
		sha1: createHash('sha1').update(randomBytes(16)).digest('hex'),
		sha256: sha256(randomBytes(16)),
		object,
		object_size: size + 16 * Math.max(1, Math.ceil(size / 1_048_576)),
		...(key ? { key: randomBytes(32).toString('hex') } : {}),
	}
}

/** A version's hash: the hash of its distinct object hashes, sorted, as raw bytes. */
function root(files) {
	const objects = [...new Set(files.map((entry) => entry.object))].sort()
	return sha256(Buffer.concat(objects.map((object) => Buffer.from(object, 'hex'))))
}

const bytes = (files) => {
	const seen = new Map(files.map((entry) => [entry.object, entry.object_size]))
	return [...seen.values()].reduce((sum, size) => sum + size, 0)
}

test('sharing API as the App and Cores use it, with files in the pool', async () => {
	const backend = await startBackend('sharing-contract', { TICKET_KEY })
	try {
		const { db, files } = backend
		for (const user of ['owner', 'friend', 'stranger', 'other']) await backend.account(user)
		await db
			.prepare('INSERT INTO friends (user_id,friend_id,accepted,created) VALUES (?,?,1,?)')
			.bind('owner', 'friend', new Date().toISOString())
			.run()
		// The owner's own Core, and a second Core owned by someone else. Neither has reported
		// its storage yet, so neither can take a share. Without a server, a share lives on
		// other people's Cores when one can take it.
		for (const [id, owner, key] of [
			['core', 'owner', 'amc_core'],
			['core2', 'other', 'amc_core2'],
		])
			await db
				.prepare(
					'INSERT INTO cores (id,owner_id,name,address,cert_fingerprint,key_hash,created,last_seen) VALUES (?,?,?,?,?,?,?,?)',
				)
				.bind(
					id,
					owner,
					id,
					`https://192.168.1.${id === 'core' ? 2 : 3}:16662`,
					'f'.repeat(64),
					createHash('sha256').update(key).digest('hex'),
					'2026-01-01',
					'2026-01-01',
				)
				.run()
		const send = (url, method, user, body, headers = {}) =>
			backend.fetch(url, {
				method,
				headers: { ...(user ? { authorization: `Bearer ${user}` } : {}), ...headers },
				body,
			})
		const call = (path, method = 'GET', body, user = 'mra_owner') =>
			send(
				`http://local.test/v1${path}`,
				method,
				user,
				body && JSON.stringify(body),
				body ? { 'content-type': 'application/json' } : {},
			)
		const ok = async (response) => {
			if (!response.ok)
				assert.fail(`expected success, got ${response.status} ${await response.text()}`)
			return response.status === 204 ? null : response.json()
		}
		const code = async (response, status, code) => {
			assert.equal(response.status, status)
			assert.equal((await response.json()).error, code)
		}
		/** `create_remote_version`: the manifest with its files already encrypted and named. */
		const push = (id, external, user = 'mra_owner') =>
			call(
				`/instances/${id}/versions`,
				'POST',
				{ ...manifest, external_files: external, version_hash: root(external) },
				user,
			)
		/** A Core's storage report, as `copal` sends it after an upload and once an hour. */
		const report = (core, versions, extra = {}) =>
			send(
				'http://local.test/core/storage',
				'POST',
				`amc_${core}`,
				JSON.stringify({
					enabled: true,
					service: false,
					cap: 20 * GB,
					used: versions.reduce((sum, entry) => sum + entry.bytes, 0),
					free: 100 * GB,
					egress_cap: 50 * GB,
					egress_today: 0,
					versions: versions.map((entry) => ({ root: entry.root, bytes: entry.bytes })),
					...extra,
				}),
				{ 'content-type': 'application/json' },
			)
		const manifests = async (id) =>
			(await files.list({ prefix: `manifests/${id}/` })).objects.map((object) => object.key).sort()
		const notifications = async () =>
			(await db.prepare('SELECT COUNT(*) AS count FROM notifications').first()).count

		// Share an instance: create it, name it, give it an icon, invite a friend.
		const { id } = await ok(await call('/instances', 'POST', { name: 'Proof' }))
		await ok(await call(`/instances/${id}`, 'PATCH', { name: 'Proof pack' }))
		const icon = randomBytes(64)
		await ok(
			await send(`http://local.test/v1/instances/${id}/icon`, 'PUT', 'mra_owner', icon, {
				'content-type': 'application/octet-stream',
			}),
		)
		const info = await ok(await call(`/instances/${id}`))
		assert.equal(info.name, 'Proof pack')
		assert.deepEqual(Buffer.from(await (await send(info.icon, 'GET')).arrayBuffer()), icon)
		assert.equal(info.installed_once, false)
		assert.equal(info.linked_server, null)
		assert.equal(info.icon_config, null, 'an uploaded image has no icon editor settings')
		// An icon made in the icon editor is only its settings, and replaces the uploaded image.
		const iconConfig = { background: { type: 'color', value: '#c78aff' }, symbol: 'dusk_block' }
		const generated = (config) =>
			send(
				`http://local.test/v1/instances/${id}/icon/generated`,
				'PUT',
				'mra_owner',
				JSON.stringify(config),
				{ 'content-type': 'application/json' },
			)
		assert.equal((await generated({ ...iconConfig, symbol: 'Not A Symbol' })).status, 400)
		await ok(await generated(iconConfig))
		const drawn = await ok(await call(`/instances/${id}`))
		assert.deepEqual([drawn.icon, drawn.icon_config], [null, iconConfig])
		assert.equal((await send(info.icon, 'GET')).status, 404, 'the replaced image is gone')
		assert.equal((await files.list({ prefix: `icons/${id}/` })).objects.length, 0)
		// Inviting a batch invites those who accept invitations and names the rest.
		assert.deepEqual(
			await ok(await call(`/instances/${id}/users`, 'POST', { user_ids: ['friend', 'stranger'] })),
			{ failed: ['stranger'] },
		)
		assert.equal(await notifications(), 0, 'no invitation before there is something to install')

		// An invite link previews the share without signing in; there is no version to show yet.
		const link = await ok(await call(`/instances/${id}/invites`, 'POST', {}))
		const preview = async (search = '?version=true') =>
			ok(await send(`http://local.test/v1/invites/${link.id}${search}`, 'GET'))
		const empty = await preview()
		assert.deepEqual(empty.inviter, { id: 'owner', name: 'owner', avatar: null })
		assert.equal(empty.version, undefined)

		// A version with only Modrinth content needs no Core and is ready at once.
		const first = await ok(await push(id, []))
		assert.equal(first.version, 1)
		assert.equal(first.ready, true)
		assert.equal(first.version_hash, EMPTY_ROOT)
		assert.deepEqual(first.external_files, [])
		assert.equal(first.upload, undefined)
		assert.equal(first.download, undefined)
		assert.deepEqual(first.reports, { content: 0 })
		assert.equal(await notifications(), 1, 'the friend is invited once a version is ready')

		// A version with files needs a Core. None has reported storage yet.
		const jar = file('private-mod.jar', 'mod', 300_000)
		const config = file('config/private-mod/client.toml', 'config', 20)
		await code(await push(id, [jar, config]), 503, 'no_storage')
		assert.deepEqual(await ok(await report('core2', [])), { drop: [], pull: [] })

		// The push: the backend keeps the manifest, picks another person's Core, and hands out a
		// store ticket. Nothing is ready until the Core confirms.
		const second = await ok(await push(id, [jar, config]))
		assert.equal(second.version, 2)
		assert.equal(second.ready, false)
		assert.equal(second.version_hash, root([jar, config]))
		assert.equal(second.download, undefined)
		assert.equal(second.upload.core.id, 'core2')
		assert.equal(second.upload.core.cert_fingerprint, 'f'.repeat(64))
		assert.ok(Date.parse(second.upload.expires) > Date.now() + 3_500_000)
		const store = decode(second.upload.ticket)
		assert.ok(store.valid)
		assert.equal(store.claims.kind, 'store')
		assert.equal(store.claims.aud, 'core2')
		assert.equal(store.claims.root, second.version_hash)
		assert.equal(store.claims.bytes, bytes([jar, config]))
		assert.equal(store.claims.exp - store.claims.iat, 3600)
		assert.deepEqual(
			second.external_files.map(
				({ file_name, file_type, url, file_size, sha1, sha256, object, object_size, key }) => ({
					file_name,
					file_type,
					url,
					file_size,
					sha1,
					sha256,
					object,
					object_size,
					key,
				}),
			),
			[jar, config].map((entry) => ({ url: null, ...entry, key: entry.key })),
		)
		assert.deepEqual(await manifests(id), [`manifests/${id}/1`, `manifests/${id}/2`])
		assert.deepEqual(
			{
				...(await db
					.prepare('SELECT home, replica FROM shared_instances WHERE id = ?')
					.bind(id)
					.first()),
			},
			{ home: 'core2', replica: null },
			'no second Core offers storage, so there is no replica yet',
		)
		await ok(await call(`/instances/${id}/invites/pending`, 'POST', undefined, 'mra_friend'))
		// The Library lists a member's joined shares, and only an owner's own without `include_joined`.
		assert.deepEqual(
			await ok(await call('/instances?user=friend', 'GET', undefined, 'mra_friend')),
			[],
		)
		assert.ok(
			(
				await ok(
					await call('/instances?user=friend&include_joined=true', 'GET', undefined, 'mra_friend'),
				)
			).includes(id),
		)
		assert.ok((await ok(await call('/instances?user=owner'))).includes(id))
		assert.equal(
			(await ok(await call(`/instances/${id}/versions`, 'GET', undefined, 'mra_friend'))).version,
			1,
			'members get the latest ready version',
		)
		assert.equal(
			(await call(`/instances/${id}/versions/2`, 'GET', undefined, 'mra_friend')).status,
			404,
			'members do not see a version before it is ready',
		)
		assert.equal(
			(await ok(await call(`/instances/${id}`, 'GET', undefined, 'mra_friend'))).installed_once,
			true,
		)

		// Pushing the same files again resumes the upload; other files wait for it.
		const resumed = await ok(await push(id, [jar, config]))
		assert.equal(resumed.version, 2)
		assert.notEqual(resumed.upload.ticket, second.upload.ticket)
		await code(await push(id, [jar]), 409, 'push_in_flight')

		// The Core confirms: the version is ready, and members get holders and a fetch ticket.
		const held = [{ root: second.version_hash, bytes: bytes([jar, config]) }]
		assert.deepEqual(await ok(await report('core2', held)), { drop: [], pull: [] })
		const latest = await ok(await call(`/instances/${id}/versions`, 'GET', undefined, 'mra_friend'))
		assert.equal(latest.version, 2)
		assert.equal(latest.ready, true)
		assert.deepEqual(
			{ ...latest, external_files: undefined, download: undefined },
			{
				...manifest,
				removed_files: [],
				version_hash: second.version_hash,
				version: 2,
				ready: true,
				external_files: undefined,
				download: undefined,
			},
		)
		assert.deepEqual(
			latest.download.holders.map((core) => core.id),
			['core2'],
		)
		const fetch = decode(latest.download.ticket)
		assert.ok(fetch.valid)
		assert.equal(fetch.claims.kind, 'fetch')
		assert.deepEqual(fetch.claims.aud, ['core2'])
		assert.equal(fetch.claims.root, second.version_hash)
		assert.equal(fetch.claims.exp - fetch.claims.iat, 6 * 3600)
		assert.equal(
			latest.external_files.find((entry) => entry.file_type === 'config').key,
			config.key,
		)
		assert.equal(latest.external_files.find((entry) => entry.file_type === 'mod').key, undefined)
		assert.equal(
			(await call(`/instances/${id}/versions`, 'GET', undefined, 'mra_stranger')).status,
			401,
		)

		// The preview shows the ready version, but never file keys or pool access.
		const shown = (await preview()).version
		assert.equal(shown.version, 2)
		assert.equal(shown.ready, true)
		assert.deepEqual(
			shown.external_files.map((entry) => entry.file_name),
			[jar.file_name, config.file_name],
		)
		assert.ok(shown.external_files.every((entry) => entry.key === undefined))
		assert.equal(shown.download, undefined)
		assert.equal(shown.upload, undefined)
		assert.doesNotMatch(JSON.stringify(shown), /ticket/)
		assert.equal((await preview('')).version, undefined)

		// A member reports broken files; the owner sees it and nobody else.
		await ok(
			await call(
				`/instances/${id}/versions/2/reports`,
				'POST',
				{ core_id: 'core', kind: 'content' },
				'mra_friend',
			),
		)
		await ok(
			await call(
				`/instances/${id}/versions/2/reports`,
				'POST',
				{ core_id: 'core', kind: 'content' },
				'mra_friend',
			),
		)
		assert.deepEqual((await ok(await call(`/instances/${id}/versions/2`))).reports, { content: 1 })
		assert.equal(
			(
				await call(
					`/instances/${id}/versions/2/reports`,
					'POST',
					{ core_id: 'core', kind: 'odd' },
					'mra_friend',
				)
			).status,
			400,
		)

		// What a manifest may not say.
		for (const [external, status] of [
			[[{ ...jar, key: randomBytes(32).toString('hex') }], 400],
			[[{ ...config, key: undefined }], 400],
			[[{ ...jar, object_size: jar.object_size + 1 }], 400],
			[[{ ...jar, sha1: undefined }], 400],
			[[{ ...jar, file_name: '../escape.jar' }], 400],
			[[jar, jar], 400],
		])
			assert.equal((await push(id, external)).status, status, JSON.stringify(external[0]))
		await code(
			await push(
				id,
				Array.from({ length: 4097 }, (_, index) => file(`${index}.jar`, 'mod', 1)),
			),
			413,
			'too_many_files',
		)
		assert.equal(
			(
				await call(`/instances/${id}/versions`, 'POST', {
					...manifest,
					external_files: [jar],
					version_hash: root([config]),
				})
			).status,
			400,
			'the root must match the files',
		)
		await code(await push(id, [file('huge.jar', 'mod', GB)]), 413, 'share_too_large')

		// An unchanged file keeps its object. Five versions are kept by default, files included:
		// the sixth ready version prunes the first manifest, and the Core is told to drop the
		// roots of versions no longer kept.
		const pushed = [second.version_hash]
		for (let push_ = 0; push_ < 5; push_++) {
			const changed = file(config.file_name, 'config', 30 + push_)
			const version = await ok(await push(id, [jar, changed]))
			assert.equal(version.ready, false)
			pushed.push(version.version_hash)
			const answer = await ok(
				await report(
					'core2',
					pushed.map((root) => ({ root, bytes: 1 })),
				),
			)
			assert.deepEqual(answer.pull, [])
			assert.deepEqual(
				answer.drop.sort(),
				pushed.slice(0, Math.max(0, pushed.length - 5)).sort(),
				'the files of kept versions stay',
			)
		}
		const current = await ok(await call(`/instances/${id}/versions`))
		assert.equal(current.version, 7)
		assert.equal(current.ready, true)
		assert.equal((await call(`/instances/${id}/versions/1`)).status, 404)
		assert.equal((await call(`/instances/${id}/versions/2`)).status, 404)
		assert.equal((await ok(await call(`/instances/${id}/versions/3`))).version, 3)
		assert.deepEqual(
			await manifests(id),
			[3, 4, 5, 6, 7].map((version) => `manifests/${id}/${version}`),
		)

		// A home that already holds the exact objects makes a version ready at once.
		const again = await ok(await report('core2', [{ root: current.version_hash, bytes: 1 }]))
		assert.deepEqual(again.drop, [])
		const same = await ok(
			await push(
				id,
				current.external_files.map(({ url: _url, ...entry }) => entry),
			),
		)
		assert.equal(same.version, 8)
		assert.equal(same.ready, true)
		assert.equal(same.version_hash, current.version_hash)

		// A replica is told to pull the kept versions it lacks from the home. Versions 7 and 8
		// have the same files, so that is one root.
		await db.prepare('UPDATE shared_instances SET replica = ? WHERE id = ?').bind('core', id).run()
		const replica = await ok(await report('core', []))
		assert.equal(replica.pull.length, 1)
		assert.equal(replica.pull[0].root, current.version_hash)
		const pull = decode(replica.pull[0].ticket)
		assert.ok(pull.valid)
		assert.equal(pull.claims.kind, 'pull')
		assert.equal(pull.claims.aud, 'core')
		assert.equal(pull.claims.source.core.id, 'core2')
		assert.equal(pull.claims.source.core.cert_fingerprint, 'f'.repeat(64))
		const inner = decode(pull.claims.source.ticket)
		assert.equal(inner.claims.kind, 'fetch')
		assert.deepEqual(inner.claims.aud, ['core2'])
		await ok(await report('core', [{ root: current.version_hash, bytes: 1 }]))
		const both = await ok(await call(`/instances/${id}/versions`, 'GET', undefined, 'mra_friend'))
		assert.deepEqual(
			both.download.holders.map((core) => core.id),
			['core2', 'core'],
		)
		assert.deepEqual(decode(both.download.ticket).claims.aud, ['core2', 'core'])

		// Leaving and removal revoke access to versions.
		await ok(await call(`/instances/${id}/users`, 'DELETE', { user_ids: ['friend'] }))
		assert.equal(
			(await call(`/instances/${id}/versions`, 'GET', undefined, 'mra_friend')).status,
			401,
		)
		assert.equal(
			(await ok(await call(`/instances/${id}`))).installed_once,
			true,
			'a share stays installed once after its last friend leaves',
		)

		// A quarantined share's invite preview shows no version.
		await db.prepare('UPDATE shared_instances SET quarantine = 1 WHERE id = ?').bind(id).run()
		assert.equal((await preview()).version, undefined)

		// Deleting a share removes its manifests.
		await ok(await call(`/instances/${id}`, 'DELETE'))
		assert.deepEqual(await manifests(id), [])

		// A 3 GB allowance on other people's Cores, across every share's kept versions.
		// Each exactly 1 GiB once encrypted: 1,024 chunks of 16 tag bytes each.
		const shares = []
		for (const name of ['A', 'B', 'C']) {
			const share = await ok(await call('/instances', 'POST', { name }))
			const big = file(`${name}.jar`, 'mod', GB - 16 * 1024)
			const version = await ok(await push(share.id, [big]))
			shares.push({ id: share.id, root: version.version_hash, big })
		}
		// Versions still uploading count too, so pushes to several shares at once cannot pass.
		const full = await ok(await call('/instances', 'POST', { name: 'Full' }))
		await code(await push(full.id, [file('one.jar', 'mod', 1)]), 413, 'storage_quota')
		await ok(
			await report(
				'core2',
				shares.map((share) => ({ root: share.root, bytes: GB })),
			),
		)
		await code(await push(full.id, [file('one.jar', 'mod', 1)]), 413, 'storage_quota')
		assert.equal(
			(await push(shares[0].id, [shares[0].big])).status,
			200,
			"replacing a share's own version is fine",
		)

		// Deleting a share removes its manifests; the Core drops its files at its next report.
		await ok(await call(`/instances/${shares[2].id}`, 'DELETE'))
		assert.equal((await call(`/instances/${shares[2].id}`)).status, 404)
		assert.deepEqual(await manifests(shares[2].id), [])
		const after = await ok(
			await report(
				'core2',
				shares.map((share) => ({ root: share.root, bytes: 1 })),
			),
		)
		assert.ok(after.drop.includes(shares[2].root))
		assert.ok(!after.drop.includes(shares[0].root))

		// A new file in B's next version adds to usage; the same file again in A's did not. Over
		// the allowance, deleting an old version frees the space.
		const usage = async () => ok(await call('/storage'))
		assert.equal((await usage()).used, 2 * GB)
		const bigger = file('B2.jar', 'mod', GB - 16 * 1024)
		const b2 = await ok(await push(shares[1].id, [bigger]))
		await ok(
			await report(
				'core2',
				[shares[0].root, shares[1].root, b2.version_hash].map((root) => ({ root, bytes: GB })),
			),
		)
		const overview = await usage()
		assert.equal(overview.used, 3 * GB)
		assert.equal(overview.allowance, 3 * GB)
		const b = overview.shares.find((share) => share.id === shares[1].id)
		assert.equal(b.counted, true)
		assert.deepEqual(
			b.versions.map(({ version, ready, current }) => ({ version, ready, current })),
			[
				{ version: 2, ready: true, current: true },
				{ version: 1, ready: true, current: false },
			],
		)
		const over = await push(full.id, [file('one.jar', 'mod', 1)])
		assert.equal(over.status, 413)
		assert.match((await over.json()).description, /Delete old versions to free space/)
		await code(
			await call(`/instances/${shares[1].id}/versions/2`, 'DELETE'),
			409,
			'current_version',
		)
		assert.equal(
			(await call(`/instances/${shares[1].id}/versions/1`, 'DELETE', undefined, 'mra_friend'))
				.status,
			401,
		)
		await ok(await call(`/instances/${shares[1].id}/versions/1`, 'DELETE'))
		assert.equal((await usage()).used, 2 * GB)
		assert.deepEqual(await manifests(shares[1].id), [`manifests/${shares[1].id}/2`])

		// An instance with a server on the owner's Core is homed there, its replica on a Core
		// friends can reach.
		const served = await ok(await call('/instances', 'POST', { name: 'Served' }))
		await ok(await call(`/instances/${served.id}/users`, 'POST', { user_ids: ['friend'] }))
		const world = file('world.jar', 'mod', 1000)
		const servedVersion = await ok(
			await call(`/instances/${served.id}/versions`, 'POST', {
				...manifest,
				external_files: [world],
				version_hash: root([world]),
				server_core: 'core',
			}),
		)
		assert.equal(servedVersion.upload.core.id, 'core')
		const placed = async () => ({
			...(await db
				.prepare('SELECT home, replica FROM shared_instances WHERE id = ?')
				.bind(served.id)
				.first()),
		})
		assert.deepEqual(await placed(), { home: 'core', replica: 'core2' })

		// A Core whose owner turns storage off runs privately: other people's shares move off it.
		await ok(await report('core2', [], { enabled: false }))
		assert.deepEqual(await placed(), { home: 'core', replica: null })

		// With no one else's Core offering storage, the owner's own takes a share without a server.
		const fallback = await ok(await call('/instances', 'POST', { name: 'Fallback' }))
		const fallbackVersion = await ok(await push(fallback.id, [file('fallback.jar', 'mod', 1000)]))
		assert.equal(fallbackVersion.upload.core.id, 'core')

		// An owner whose Core runs privately keeps every share there alone, using no allowance.
		await ok(await report('core', [], { enabled: false }))
		const alone = await ok(await call('/instances', 'POST', { name: 'Alone' }))
		const aloneVersion = await ok(await push(alone.id, [file('alone.jar', 'mod', 1000)]))
		assert.equal(aloneVersion.upload.core.id, 'core')
		const aloneShare = (await usage()).shares.find((share) => share.id === alone.id)
		assert.deepEqual([aloneShare.counted, aloneShare.used], [false, 0])

		// Players of a server read the share it follows without being members, until removed.
		const smp = await ok(await call('/instances', 'POST', { name: 'SMP' }))
		await ok(await push(smp.id, []))
		const server = { instance_id: smp.id, name: 'SMP server', port: 25570 }
		assert.deepEqual(
			await ok(
				await call('/servers/core/srv1/players', 'POST', { ...server, user_ids: ['friend'] }),
			),
			{ failed: [] },
		)
		const asFriend = (path) => call(path, 'GET', undefined, 'mra_friend')
		assert.equal((await asFriend(`/instances/${smp.id}/versions`)).status, 401, 'until accepted')
		await ok(await call(`/instances/${smp.id}/invites/pending`, 'POST', undefined, 'mra_friend'))
		assert.equal((await ok(await asFriend(`/instances/${smp.id}/versions`))).version, 1)
		// Players join at the Core's last reported address with the server's port.
		const linkedDomain = async () =>
			(await ok(await asFriend(`/instances/${smp.id}`))).linked_server.domain
		assert.deepEqual((await ok(await asFriend(`/instances/${smp.id}`))).linked_server, {
			domain: '192.168.1.2:25570',
			region: '',
			online_status: null,
		})
		await db
			.prepare('UPDATE cores SET public_address = ? WHERE id = ?')
			.bind('https://203.0.113.7:16662', 'core')
			.run()
		assert.equal(await linkedDomain(), '203.0.113.7:25570', 'the outside address once reported')
		// The router's port is used while it is on the Core's public IP, and a write without one
		// keeps it. A stale outside on another IP loses to the public address the Core reports.
		const writePlayers = (outside) =>
			call('/servers/core/srv1/players', 'POST', { ...server, outside, user_ids: [] })
		await ok(await writePlayers({ ip: '203.0.113.7', port: 40000 }))
		assert.equal(await linkedDomain(), '203.0.113.7:40000')
		await ok(await writePlayers(null))
		assert.equal(await linkedDomain(), '203.0.113.7:40000')
		await ok(await writePlayers({ ip: '198.51.100.4', port: 40000 }))
		assert.equal(await linkedDomain(), '203.0.113.7:25570')
		assert.equal((await writePlayers({ ip: 'evil.example', port: 1 })).status, 400)
		await db.prepare('UPDATE hosted_servers SET outside = NULL').run()
		await db.prepare('UPDATE cores SET public_address = NULL WHERE id = ?').bind('core').run()
		assert.deepEqual(
			(await ok(await call(`/instances/${smp.id}/users`))).users.map((user) => user.id),
			['owner'],
			'a player is not a member',
		)
		assert.deepEqual(
			(await ok(await call('/servers/core/srv1/players'))).users.map((user) => user.id),
			['friend'],
		)
		// A member by the share's link, and a player by the server's link.
		const shareLink = await ok(await call(`/instances/${smp.id}/invites`, 'POST', {}))
		await ok(
			await call(`/instances/${smp.id}/invites/${shareLink.id}`, 'POST', undefined, 'mra_stranger'),
		)
		const serverLink = await ok(
			await call('/servers/core/srv1/invites', 'POST', { ...server, max_uses: 5 }),
		)
		assert.deepEqual(
			(await ok(await call(`/instances/${smp.id}/invites`))).map((invite) => invite.id),
			[shareLink.id],
			"a server's links are listed with its players",
		)
		await ok(
			await call(`/instances/${smp.id}/invites/${serverLink.id}`, 'POST', undefined, 'mra_other'),
		)
		assert.equal(
			(await ok(await call(`/instances/${smp.id}/versions`, 'GET', undefined, 'mra_other')))
				.version,
			1,
		)
		// A member joins the share, a player the server.
		assert.deepEqual(
			(await ok(await call(`/instances/${smp.id}/users`))).users.map((user) => user.id),
			['owner', 'stranger'],
		)
		assert.deepEqual(
			(await ok(await call('/servers/core/srv1/players'))).users.map((user) => user.id),
			['friend', 'other'],
		)
		// Only the Core's owner or the share's owner removes a player; removal ends their access.
		assert.equal(
			(await call('/servers/core/srv1/players', 'DELETE', { user_ids: ['friend'] }, 'mra_stranger'))
				.status,
			403,
		)
		await ok(await call('/servers/core/srv1/players', 'DELETE', { user_ids: ['friend'] }))
		assert.equal((await asFriend(`/instances/${smp.id}/versions`)).status, 401)
		assert.equal((await asFriend(`/instances/${smp.id}`)).status, 401)
		// A member's server passes the share on to its players only while its owner is a member.
		await ok(
			await call(`/instances/${smp.id}/invites/${shareLink.id}`, 'POST', undefined, 'mra_other'),
		)
		const memberServer = { instance_id: smp.id, name: 'Other SMP', port: 25565 }
		const memberLink = await ok(
			await call(
				'/servers/core2/srv2/invites',
				'POST',
				{ ...memberServer, max_uses: 5 },
				'mra_other',
			),
		)
		// Its link shows the server and its owner, not the instance's members.
		const serverPreview = await ok(
			await send(`http://local.test/v1/invites/${memberLink.id}`, 'GET'),
		)
		assert.deepEqual(
			[
				serverPreview.instance_id,
				serverPreview.instance_name,
				serverPreview.inviter.id,
				serverPreview.instance_users,
			],
			[smp.id, 'Other SMP', 'other', []],
		)
		await ok(
			await call(`/instances/${smp.id}/invites/${memberLink.id}`, 'POST', undefined, 'mra_friend'),
		)
		assert.equal((await ok(await asFriend(`/instances/${smp.id}/versions`))).version, 1)
		// Players do not see the member list.
		await code(await asFriend(`/instances/${smp.id}/users`), 403, 'members_only')
		// Each Core's owner sees their server's links.
		assert.deepEqual(
			(await ok(await call('/servers/core2/srv2/invites', 'GET', undefined, 'mra_other'))).map(
				(invite) => invite.id,
			),
			[memberLink.id],
		)
		assert.deepEqual(
			(await ok(await call('/servers/core/srv1/invites'))).map((invite) => invite.id),
			[serverLink.id],
		)
		// Checked when read: a member whose membership lapses no longer passes it on.
		const setJoined = (value) =>
			db
				.prepare(
					'UPDATE shared_instance_users SET joined_at = ? WHERE instance_id = ? AND user_id = ?',
				)
				.bind(value, smp.id, 'other')
				.run()
		await setJoined(null)
		assert.equal((await asFriend(`/instances/${smp.id}/versions`)).status, 401)
		await setJoined(new Date().toISOString())
		assert.equal((await asFriend(`/instances/${smp.id}/versions`)).status, 200)
		// Kicking the member drops their server, its players, and its links.
		await ok(await call(`/instances/${smp.id}/users`, 'DELETE', { user_ids: ['other'] }))
		assert.equal((await asFriend(`/instances/${smp.id}/versions`)).status, 401)
		assert.equal(
			await db
				.prepare('SELECT 1 FROM hosted_servers WHERE core_id = ? AND server_id = ?')
				.bind('core2', 'srv2')
				.first(),
			null,
		)
		assert.equal((await send(`http://local.test/v1/invites/${memberLink.id}`, 'GET')).status, 404)
		// A Core that deletes or unlinks a server drops its players and links.
		assert.equal(
			(await call(`/instances/${smp.id}/versions`, 'GET', undefined, 'mra_other')).status,
			200,
			'still a player of srv1',
		)
		const forget = await send(
			'http://local.test/core/players',
			'DELETE',
			'amc_core',
			JSON.stringify({ server_id: 'srv1' }),
			{ 'content-type': 'application/json' },
		)
		assert.equal(forget.status, 204)
		assert.equal(
			(await call(`/instances/${smp.id}/versions`, 'GET', undefined, 'mra_other')).status,
			401,
		)
		assert.equal((await send(`http://local.test/v1/invites/${serverLink.id}`, 'GET')).status, 404)
		// A server that follows another share drops its players: they were given the old one.
		const asOther = (path, method = 'GET') => call(path, method, undefined, 'mra_other')
		const rejoin = await ok(
			await call('/servers/core/srv1/invites', 'POST', { ...server, max_uses: 1 }),
		)
		await ok(await asOther(`/instances/${smp.id}/invites/${rejoin.id}`, 'POST'))
		assert.equal((await asOther(`/instances/${smp.id}/versions`)).status, 200)
		const next = await ok(await call('/instances', 'POST', { name: 'Next' }))
		await ok(
			await call('/servers/core/srv1/invites', 'POST', {
				...server,
				instance_id: next.id,
				max_uses: 1,
			}),
		)
		assert.equal((await asOther(`/instances/${smp.id}/versions`)).status, 401)
		assert.deepEqual((await ok(await call('/servers/core/srv1/players'))).users, [])
		// The Core's late forget of the old share keeps the players of the one it follows now.
		await ok(
			await call('/servers/core/srv1/players', 'POST', {
				...server,
				instance_id: next.id,
				user_ids: ['friend'],
			}),
		)
		const forgetShare = (share_id) =>
			send(
				'http://local.test/core/players',
				'DELETE',
				'amc_core',
				JSON.stringify({ server_id: 'srv1', share_id }),
				{ 'content-type': 'application/json' },
			)
		assert.equal((await forgetShare(smp.id)).status, 204)
		const players = async () =>
			(await ok(await call('/servers/core/srv1/players'))).users.map((user) => user.id)
		assert.deepEqual(await players(), ['friend'])
		assert.equal((await forgetShare(next.id)).status, 204)
		assert.deepEqual(await players(), [])

		// A Core writes the share of its own server-first server with its key.
		const asCore = (path, method, body, core = 'core') => call(path, method, body, `amc_${core}`)
		const serverShare = await ok(
			await asCore('/instances', 'POST', { name: 'Server', server_id: 'server' }),
		)
		assert.deepEqual(
			await ok(await asCore('/instances', 'POST', { name: 'Server', server_id: 'server' })),
			serverShare,
		)
		// A server installed from a modpack shares it, with the pack files players do not get.
		const removed_files = [
			{ type: 'version', version_id: 'PACKMOD1' },
			{ type: 'path', parent: 'mods', filename: 'server-only.jar' },
		]
		const packVersion = (body) =>
			asCore(`/instances/${serverShare.id}/versions`, 'POST', {
				...manifest,
				external_files: [],
				removed_files,
				...body,
			})
		assert.equal((await packVersion({})).status, 400)
		const outside = { type: 'path', parent: 'config', filename: 'a.toml' }
		const notAddon = { type: 'path', parent: 'mods', filename: 'notes.txt' }
		// The App refuses a name that is not one safe file name.
		const backslash = { type: 'path', parent: 'mods', filename: 'a\\b.jar' }
		const device = { type: 'path', parent: 'mods', filename: 'con.jar' }
		for (const bad of [outside, notAddon, backslash, device])
			assert.equal(
				(await packVersion({ modpack_id: 'PACKVER1', removed_files: [bad] })).status,
				400,
			)
		await ok(await packVersion({ modpack_id: 'PACKVER1' }))
		const shared = await ok(await asCore(`/instances/${serverShare.id}/versions`, 'GET'))
		assert.equal(shared.modpack_id, 'PACKVER1')
		assert.deepEqual(shared.removed_files, removed_files)
		// A Core picks its own id when it pairs, so a matching core_id is not enough: a share whose
		// owner is not the Core's owner stays out of its reach.
		const moveShare = (core) =>
			db
				.prepare('UPDATE shared_instances SET core_id = ? WHERE id = ?')
				.bind(core, serverShare.id)
				.run()
		await moveShare('core2')
		await code(
			await asCore(`/instances/${serverShare.id}`, 'PATCH', { name: 'Taken' }, 'core2'),
			403,
			'not_owner',
		)
		await code(
			await asCore(`/instances/${serverShare.id}/versions`, 'GET', undefined, 'core2'),
			401,
			'not_member',
		)
		await code(
			await asCore('/instances', 'POST', { name: 'Taken', server_id: 'server' }, 'core2'),
			403,
			'not_owner',
		)
		await moveShare('core')
		// An owner who may not share any more can still delete a share.
		const leaving = await ok(
			await asCore('/instances', 'POST', { name: 'Leaving', server_id: 'leaving' }),
		)
		await db.prepare('INSERT INTO sharing_blacklist (user_id) VALUES (?)').bind('owner').run()
		await code(
			await asCore(`/instances/${leaving.id}`, 'PATCH', { name: 'Still' }),
			403,
			'blacklisted',
		)
		await ok(await asCore(`/instances/${leaving.id}`, 'DELETE'))
		await db.prepare('DELETE FROM sharing_blacklist WHERE user_id = ?').bind('owner').run()

		// A server following a share: the Core reads it for the user who asked, with the read
		// ticket that user's App got for this Core and share. Without one it reads as its owner.
		const readFor = (share, ticket, core = 'core') =>
			send(
				`http://local.test/v1/instances/${share}/versions${ticket ? `?read_ticket=${encodeURIComponent(ticket)}` : ''}`,
				'GET',
				`amc_${core}`,
			)
		const readTicket = async (share, user, core = 'core') => {
			const { ticket } = await ok(
				await call(`/instances/${share}/server-ticket`, 'POST', { core_id: core }, `mra_${user}`),
			)
			assert.deepEqual(
				(({ kind, sub, aud, share }) => ({ kind, sub, aud, share }))(decode(ticket).claims),
				{ kind: 'read', sub: user, aud: core, share },
			)
			return ticket
		}
		// The Core's owner is a member of the stranger's share, so the Core reads it without a
		// ticket, as for its CLI.
		const strangers = await ok(
			await call('/instances', 'POST', { name: 'Private' }, 'mra_stranger'),
		)
		const join = (share, user) =>
			db
				.prepare(
					"INSERT INTO shared_instance_users (instance_id,user_id,join_type,joined_at) VALUES (?,?,'invite',?)",
				)
				.bind(share, user, new Date().toISOString())
				.run()
		await join(strangers.id, 'owner')
		await code(await readFor(strangers.id), 404, 'version_not_found')
		// A Core that wrote a user into its access list still cannot read as them without their
		// ticket: it reads as its owner, who is not a member of the friend's share.
		await db
			.prepare("INSERT INTO core_access (core_id,user_id,created) VALUES ('core','friend',?)")
			.bind(new Date().toISOString())
			.run()
		const friends = await ok(await call('/instances', 'POST', { name: 'Mine' }, 'mra_friend'))
		await code(await readFor(friends.id), 401, 'not_member')
		// A forged ticket, or one signed by another key, is refused.
		await code(await readFor(friends.id, 'forged'), 403, 'ticket_invalid')
		const other = generateKeyPairSync('ed25519').privateKey
		const encode = (value) => Buffer.from(JSON.stringify(value)).toString('base64url')
		const exp = Math.floor(Date.now() / 1000) + 300
		const unsigned = `${encode({ alg: 'EdDSA', typ: 'JWT', kid })}.${encode({ iss: 'amberite', kind: 'read', sub: 'friend', aud: 'core', share: friends.id, exp })}`
		const forged = `${unsigned}.${sign(null, Buffer.from(unsigned), other).toString('base64url')}`
		await code(await readFor(friends.id, forged), 403, 'ticket_invalid')
		// With the user's own ticket the Core reads their share, and only that share on that Core.
		const friendTicket = await readTicket(friends.id, 'friend')
		await code(await readFor(friends.id, friendTicket), 404, 'version_not_found')
		await code(await readFor(strangers.id, friendTicket), 403, 'ticket_invalid')
		await code(await readFor(friends.id, friendTicket, 'core2'), 403, 'ticket_invalid')
		await code(
			await readFor(friends.id, await readTicket(friends.id, 'friend', 'core2')),
			403,
			'ticket_invalid',
		)
		// Only someone who can read a share gets a ticket for it, and a member's ticket reads only
		// while they still can.
		await code(
			await call(
				`/instances/${strangers.id}/server-ticket`,
				'POST',
				{ core_id: 'core' },
				'mra_friend',
			),
			401,
			'not_member',
		)
		await join(strangers.id, 'friend')
		const memberTicket = await readTicket(strangers.id, 'friend')
		await code(await readFor(strangers.id, memberTicket), 404, 'version_not_found')
		await db
			.prepare("DELETE FROM shared_instance_users WHERE instance_id = ? AND user_id = 'friend'")
			.bind(strangers.id)
			.run()
		await code(await readFor(strangers.id, memberTicket), 401, 'not_member')
		// A Core's read gets no owner extras, even of its owner's share still uploading.
		const coreRead = await ok(await readFor(alone.id))
		assert.equal(coreRead.ready, false)
		assert.equal(coreRead.upload, undefined)
		assert.equal(coreRead.reports, undefined)
		assert.ok((await ok(await call(`/instances/${alone.id}/versions`))).upload)

		// Removing the Core from the account deletes its server-first shares.
		await ok(await send('http://local.test/cores/core', 'DELETE', 'mra_owner'))
		assert.equal(
			await db.prepare('SELECT 1 FROM shared_instances WHERE id = ?').bind(serverShare.id).first(),
			null,
		)
		// Modpack files the owner turned off are kept with the version, by the App's own rules.
		const pack = await ok(await call('/instances', 'POST', { name: 'Pack' }))
		const removed = [
			{ type: 'version', version_id: 'AAAABBBB' },
			{ type: 'path', parent: 'mods', filename: 'extra.jar.disabled' },
		]
		const withRemoved = (removed_files, modpack_id = 'PACKPACK') =>
			call(`/instances/${pack.id}/versions`, 'POST', {
				...manifest,
				modpack_id,
				removed_files,
				external_files: [],
				version_hash: EMPTY_ROOT,
			})
		for (const [bad, modpack] of [
			[[removed[0]], null],
			[[{ type: 'path', parent: 'config', filename: 'a.jar' }]],
			[[{ type: 'path', parent: 'mods', filename: 'a.txt' }]],
			[[{ type: 'path', parent: 'mods', filename: '../a.jar' }]],
			[[{ type: 'other' }]],
		])
			assert.equal((await withRemoved(bad, modpack)).status, 400, JSON.stringify(bad))
		await ok(await withRemoved(removed))
		assert.deepEqual(
			(await ok(await call(`/instances/${pack.id}/versions`))).removed_files,
			removed,
		)
	} finally {
		await backend.dispose()
	}
})
