import assert from 'node:assert/strict'
import { createHash, generateKeyPairSync, verify } from 'node:crypto'
import test from 'node:test'

import { startBackend } from './worker.mjs'

const { privateKey, publicKey } = generateKeyPairSync('ed25519')
const kid = 'test-key'
const TICKET_KEY = JSON.stringify({ ...privateKey.export({ format: 'jwk' }), kid })

/** A backend with accounts `owner`, `friend`, `other` and `stranger`, and Core `core` owned by `owner`. */
async function withCore(name, run) {
	const backend = await startBackend(name, { TICKET_KEY })
	try {
		for (const user of ['owner', 'friend', 'other', 'stranger']) await backend.account(user)
		await backend.db
			.prepare(
				'INSERT INTO cores (id,owner_id,name,address,key_hash,created,last_seen) VALUES (?,?,?,?,?,?,?)',
			)
			.bind(
				'core',
				'owner',
				'Core',
				'https://192.168.1.2:16662',
				createHash('sha256').update('amc_test').digest('hex'),
				'2026-01-01',
				'2026-01-01',
			)
			.run()
		// Every row written to the tables a Core's reports touch, so a test can tell "nothing changed"
		// from "rewrote the same values".
		await backend.db.prepare('CREATE TABLE writes (tbl TEXT NOT NULL)').run()
		for (const [table, event] of [
			['cores', 'UPDATE'],
			['visible_servers', 'INSERT'],
			['visible_servers', 'DELETE'],
			['core_access', 'INSERT'],
			['core_access', 'DELETE'],
		])
			await backend.db
				.prepare(
					`CREATE TRIGGER count_${table}_${event} AFTER ${event} ON ${table} BEGIN INSERT INTO writes VALUES ('${table}'); END`,
				)
				.run()
		const call = (method, path, body, token = 'amc_test') =>
			backend.fetch(`http://local.test${path}`, {
				method,
				headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' },
				body: body === undefined ? undefined : JSON.stringify(body),
			})
		const writes = async () =>
			(await backend.db.prepare('SELECT COUNT(*) AS n FROM writes').first()).n
		await run({ backend, call, writes })
	} finally {
		await backend.dispose()
	}
}

function decode(ticket) {
	const [header, claims, signature] = ticket.split('.')
	const json = (part) => JSON.parse(Buffer.from(part, 'base64url').toString())
	return {
		header: json(header),
		claims: json(claims),
		valid: verify(
			null,
			Buffer.from(`${header}.${claims}`),
			publicKey,
			Buffer.from(signature, 'base64url'),
		),
	}
}

test('tickets carry the user and the Core, for the owner and the access directory only', async () => {
	await withCore('core-tickets', async ({ backend, call, writes }) => {
		await backend.db
			.prepare("INSERT INTO core_access (core_id,user_id,created) VALUES ('core','friend','x')")
			.run()
		const before = await writes()

		const response = await call('POST', '/cores/core/ticket', undefined, 'mra_owner')
		assert.equal(response.status, 200)
		const { header, claims, valid } = decode((await response.json()).ticket)
		assert.ok(valid, 'the signature verifies with the public key')
		assert.deepEqual(header, { alg: 'EdDSA', typ: 'JWT', kid })
		assert.equal(claims.iss, 'amberite')
		assert.equal(claims.sub, 'owner')
		assert.equal(claims.name, 'owner')
		assert.equal(claims.avatar_url, null)
		assert.equal(claims.aud, 'core')
		assert.equal(claims.exp - claims.iat, 300)
		assert.ok(Math.abs(claims.iat - Date.now() / 1000) < 60)
		assert.equal(typeof claims.jti, 'string')

		const friend = await call('POST', '/cores/core/ticket', undefined, 'mra_friend')
		assert.equal(friend.status, 200)
		const friendClaims = decode((await friend.json()).ticket).claims
		assert.equal(friendClaims.sub, 'friend')
		assert.notEqual(friendClaims.jti, claims.jti)

		assert.equal((await call('POST', '/cores/core/ticket', undefined, 'mra_stranger')).status, 404)
		assert.equal((await call('POST', '/cores/nope/ticket', undefined, 'mra_owner')).status, 404)
		assert.equal((await call('POST', '/cores/core/ticket', undefined, 'amc_test')).status, 401)
		assert.equal(await writes(), before)
	})
})

test('PUT /core/access replaces the directory, writing only what changed', async () => {
	await withCore('core-access-replace', async ({ backend, call, writes }) => {
		await backend.db
			.prepare(
				"INSERT INTO core_access (core_id,user_id,created) VALUES ('core','friend','x'), ('core','other','kept')",
			)
			.run()
		const before = await writes()
		const directory = { user_ids: ['other', 'stranger', 'owner', 'nobody'] }
		assert.equal((await call('PUT', '/core/access', directory)).status, 204)
		const rows = (
			await backend.db
				.prepare("SELECT user_id, created FROM core_access WHERE core_id = 'core' ORDER BY user_id")
				.all()
		).results
		assert.deepEqual(
			rows.map((row) => row.user_id),
			['other', 'stranger'],
		)
		assert.equal(rows[0].created, 'kept', 'an unchanged row is not rewritten')
		assert.equal(await writes(), before + 2, 'one removal and one addition')

		assert.equal((await call('PUT', '/core/access', directory)).status, 204)
		assert.equal(await writes(), before + 2, 'the same directory again writes nothing')
		assert.equal((await call('POST', '/cores/core/ticket', undefined, 'mra_friend')).status, 404)

		assert.equal((await call('PUT', '/core/access', { user_ids: [7] })).status, 400)
		const removed = await call('PUT', '/core/access', directory, 'amc_unknown')
		assert.equal(removed.status, 410)
		assert.equal((await removed.json()).error, 'core_removed')
	})
})

test('state reports write only what changed', async () => {
	await withCore('core-state', async ({ backend, call, writes }) => {
		await backend.db
			.prepare(
				"INSERT INTO shared_instances (id,owner_id,name,created) VALUES ('pack','owner','Pack','x')",
			)
			.run()
		await backend.db
			.prepare(
				"INSERT INTO shared_members (instance_id,user_id,joined_at,join_type) VALUES ('pack','owner','x','owner')",
			)
			.run()
		const server = {
			id: 's1',
			name: 'SMP',
			address: '203.0.113.5:25565',
			shared_instance_id: 'pack',
		}
		const state = {
			version: '0.1.0',
			address: 'https://192.168.1.2:16662',
			public_address: null,
			cert_fingerprint: 'a'.repeat(64),
			servers: [server],
		}
		const core = () => backend.db.prepare("SELECT * FROM cores WHERE id = 'core'").first()
		const visible = async () =>
			(await backend.db.prepare("SELECT * FROM visible_servers WHERE core_id = 'core'").all())
				.results

		const before = await writes()
		assert.equal((await call('POST', '/core/state', state)).status, 204)
		assert.equal((await core()).cert_fingerprint, 'a'.repeat(64))
		assert.equal((await core()).version, '0.1.0')
		assert.equal((await visible()).length, 1)
		const first = await writes()
		assert.ok(first > before)

		assert.equal((await call('POST', '/core/state', state)).status, 204)
		assert.equal(await writes(), first, 'the same report again writes nothing')

		const moved = { ...state, public_address: 'https://203.0.113.5:16662' }
		assert.equal((await call('POST', '/core/state', moved)).status, 204)
		assert.equal((await core()).public_address, 'https://203.0.113.5:16662')
		assert.equal(await writes(), first + 1, 'one changed field is one write')

		assert.equal((await call('POST', '/core/state', { ...moved, servers: [] })).status, 204)
		assert.equal((await visible()).length, 0)

		const plain = { ...state, address: 'http://192.168.1.2:16662' }
		assert.equal((await call('POST', '/core/state', plain)).status, 400)
		assert.equal(
			(await call('POST', '/core/state', { ...state, cert_fingerprint: null })).status,
			400,
		)

		assert.equal((await call('DELETE', '/cores/core', undefined, 'mra_owner')).status, 204)
		const removed = await call('POST', '/core/state', state)
		assert.equal(removed.status, 410)
		assert.equal((await removed.json()).error, 'core_removed')
	})
})

test('only the owner invites', async () => {
	await withCore('core-invites', async ({ backend, call }) => {
		await backend.db
			.prepare("INSERT INTO core_access (core_id,user_id,created) VALUES ('core','friend','x')")
			.run()
		const invite = { server_id: 'server', server_name: 'Server', role: 'editor', user: 'other' }
		const byFriend = await call('POST', '/core/access', { ...invite, invited_by: 'friend' })
		assert.equal(byFriend.status, 403)
		assert.equal((await call('POST', '/core/access', invite)).status, 200)
		const notification = await backend.db
			.prepare("SELECT body FROM notifications WHERE user_id = 'other'")
			.first()
		assert.equal(JSON.parse(notification.body).invited_by, 'owner')
		// Core allows 64 characters; outside the BMP that is 128 UTF-16 units.
		const wide = { ...invite, server_name: '\u{1F7E7}'.repeat(64) }
		assert.equal((await call('POST', '/core/access', wide)).status, 200)
		assert.ok(
			await backend.db
				.prepare("SELECT 1 FROM core_access WHERE core_id = 'core' AND user_id = 'other'")
				.first(),
		)
	})
})
