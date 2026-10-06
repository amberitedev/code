import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import test from 'node:test'

// Exercise the real Worker and D1 SQL without account-provider or storage-node fixtures.
const require = createRequire(import.meta.url)
const wranglerRequire = createRequire(require.resolve('wrangler'))
const { Miniflare } = wranglerRequire('miniflare')
const { build } = wranglerRequire('esbuild')

test('pending sharing, account isolation, idempotent publish, and immutable uploads', async () => {
	const bundled = await build({
		stdin: {
			contents: `import {handleSharing} from './index'; import {ApiError,json} from '../common/http'; export {FriendsHub} from '../social/socket'; export default {async fetch(request,env){try{return await handleSharing(request,env)??new Response(null,{status:404})}catch(error){if(error instanceof ApiError)return json({error:error.error},error.status);throw error}}}`,
			resolveDir: fileURLToPath(new URL('.', import.meta.url)),
			loader: 'ts',
		},
		bundle: true,
		write: false,
		format: 'esm',
		platform: 'browser',
		target: 'es2022',
		external: ['cloudflare:workers'],
	})
	const runtime = new Miniflare({
		modules: true,
		script: bundled.outputFiles[0].text,
		compatibilityDate: '2026-07-22',
		compatibilityFlags: ['nodejs_compat'],
		d1Databases: { DB: 'sharing-contract' },
		durableObjects: { FRIENDS: { className: 'FriendsHub', useSQLite: true } },
		bindings: { STORAGE_NODES: '[]' },
	})
	try {
		const db = await runtime.getD1Database('DB')
		for (const file of [
			'0001_accounts.sql',
			'0002_sharing.sql',
			'0003_sharing_idempotency.sql',
			'0004_account_locks.sql',
		]) {
			const migration = await readFile(new URL(`../../migrations/${file}`, import.meta.url), 'utf8')
			for (const statement of migration
				.split(';')
				.map((value) => value.trim())
				.filter(Boolean))
				await db.prepare(statement).run()
		}
		for (const user of ['owner', 'friend', 'stranger']) {
			await db
				.prepare('INSERT INTO users (id,username,email,created) VALUES (?,?,?,?)')
				.bind(user, user, `${user}@local.invalid`, new Date().toISOString())
				.run()
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
		await db
			.prepare('INSERT INTO friends (user_id,friend_id,accepted,created) VALUES (?,?,1,?)')
			.bind('owner', 'friend', new Date().toISOString())
			.run()
		const call = (path, method = 'GET', body, user = 'owner', headers = {}) =>
			runtime.dispatchFetch(`http://local.test/v1${path}`, {
				method,
				headers: {
					authorization: `Bearer mra_${user}`,
					...headers,
					...(body ? { 'content-type': 'application/json' } : {}),
				},
				...(body ? { body: JSON.stringify(body) } : {}),
			})
		const created = await call('/instances', 'POST', { name: 'Resource pack proof' })
		assert.equal(created.status, 200)
		const { id } = await created.json()
		assert.equal(
			(await call(`/instances/${id}/users`, 'POST', { user_ids: ['friend'] })).status,
			204,
		)
		assert.equal(
			(await db.prepare('SELECT COUNT(*) AS count FROM notifications').first()).count,
			0,
			'Pending bytes must not produce an install invitation',
		)
		assert.equal((await call(`/instances/${id}`, 'GET', undefined, 'stranger')).status, 401)
		const request = {
			game_version: '1.21.1',
			loader: 'vanilla',
			loader_version: '',
			modrinth_ids: ['public-version'],
			external_files: [{ file_name: 'textures.zip', file_type: 'resourcepack' }],
		}
		const first = await call(`/instances/${id}/versions`, 'POST', request, 'owner', {
			'Idempotency-Key': 'publish-retry',
		})
		assert.equal(first.status, 200)
		const version = await first.json()
		assert.equal(version.ready, false)
		assert.equal(version.external_files[0].file_type, 'resourcepack')
		const repeated = await (
			await call(`/instances/${id}/versions`, 'POST', request, 'owner', {
				'Idempotency-Key': 'publish-retry',
			})
		).json()
		assert.equal(repeated.version, version.version)
		assert.equal(repeated.external_files[0].url, version.external_files[0].url)
		assert.equal(
			(
				await call(
					`/instances/${id}/versions`,
					'POST',
					{ ...request, game_version: '1.21.2' },
					'owner',
					{ 'Idempotency-Key': 'publish-retry' },
				)
			).status,
			409,
		)
		const token = new URL(version.external_files[0].url).pathname.split('/').at(-1)
		assert.equal(
			(
				await call(
					`/uploads/${token}/prepare`,
					'POST',
					{ sha256: 'a'.repeat(64), size: 12 },
					'friend',
				)
			).status,
			403,
		)
		const prepare = await (
			await call(`/uploads/${token}/prepare`, 'POST', { sha256: 'a'.repeat(64), size: 12 })
		).json()
		assert.equal(prepare.status, 'pending', 'No storage node means a persistent queued upload')
		assert.equal(
			(await call(`/uploads/${token}/prepare`, 'POST', { sha256: 'b'.repeat(64), size: 12 }))
				.status,
			409,
		)
		assert.equal((await (await call(`/uploads/${token}/status`)).json()).status, 'pending')
		assert.equal(
			(await call('/storage/receipt', 'POST', { node_id: 'fake', receipt: 'invalid' })).status,
			401,
		)
		assert.equal((await (await call(`/instances/${id}/versions`)).json()).ready, false)
		assert.equal((await db.prepare('SELECT COUNT(*) AS count FROM notifications').first()).count, 0)
		assert.equal(
			(await call(`/instances/${id}/versions`, 'POST', { ...request, external_files: [] })).status,
			200,
		)
		assert.equal(
			(await db.prepare('SELECT COUNT(*) AS count FROM notifications').first()).count,
			1,
			'Public-only version can become available immediately',
		)
		assert.equal(
			(await call(`/instances/${id}/invites/pending`, 'POST', undefined, 'friend')).status,
			204,
		)
		assert.equal(
			(await (await call(`/instances/${id}/versions`, 'GET', undefined, 'friend')).json()).ready,
			true,
		)
		assert.equal(
			(await call(`/instances/${id}/users`, 'DELETE', { user_ids: ['friend'] })).status,
			204,
		)
		assert.equal((await call(`/instances/${id}/versions`, 'GET', undefined, 'friend')).status, 401)
	} finally {
		await runtime.dispose()
	}
})
