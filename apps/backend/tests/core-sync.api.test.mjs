import assert from 'node:assert/strict'
import { createHash, generateKeyPairSync, verify } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import test from 'node:test'

import { startBackend } from './worker.mjs'

const { privateKey, publicKey } = generateKeyPairSync('ed25519')
const kid = 'test-key'
const TICKET_KEY = JSON.stringify({ ...privateKey.export({ format: 'jwk' }), kid })
/**
 * A session ticket signed here, which Core's ticket test verifies. It lives in Core's folder so
 * that a change to it runs that test. `WRITE_TICKET=1` signs a new one.
 */
const SIGNED_TICKET = new URL('../../core/src/access/session-ticket.json', import.meta.url)

/** A backend with accounts `owner` and `friend`, and Core `core` owned by `owner`. */
async function withCore(name, run) {
	const backend = await startBackend(name, { TICKET_KEY })
	try {
		for (const user of ['owner', 'friend']) await backend.account(user)
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
		for (const [table, event] of [['cores', 'UPDATE']])
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

test('a signed-out Core waits on a socket that approval closes', async () => {
	await withCore('core-sign-in', async ({ backend, call }) => {
		const { code, key } = await (
			await call(
				'POST',
				'/core/auth/code',
				{ core_id: 'new-core', name: 'Box', address: 'https://192.168.1.3:16662' },
				'',
			)
		).json()
		const wait = () =>
			backend.fetch('http://local.test/core/auth/wait', {
				headers: { upgrade: 'websocket', authorization: `Bearer ${key}` },
			})
		const waiting = await wait()
		assert.equal(waiting.status, 101)
		waiting.webSocket.accept()
		const woken = new Promise((resolve) =>
			waiting.webSocket.addEventListener('message', (event) => resolve(event.data)),
		)

		assert.equal((await call('POST', '/cores/approve', { code }, 'mra_owner')).status, 200)
		assert.equal(await woken, 'approved')
		assert.deepEqual(await (await call('POST', '/core/auth/poll', undefined, key)).json(), {
			status: 'approved',
			account: { id: 'owner', username: 'owner' },
		})
		// The code is used up, so there is nothing left to wait for.
		assert.equal((await wait()).status, 410)
	})
})

test('the backend introduces the owner to a Core over its link, and writes nothing', async () => {
	await withCore('core-link', async ({ backend, call, writes }) => {
		const before = await writes()
		const dialer = { id: 'ab'.repeat(32), candidates: ['203.0.113.9:40000', '[2001:db8::1]:40000'] }
		const introduce = (token, body = dialer) => call('POST', '/cores/core/introduce', body, token)

		// No link: the Core is off, or cannot reach the backend.
		const off = await introduce('mra_owner')
		assert.equal(off.status, 503)
		assert.equal((await off.json()).error, 'core_offline')

		const link = await backend.fetch('http://local.test/core/link', {
			headers: { upgrade: 'websocket', authorization: 'Bearer amc_test' },
		})
		assert.equal(link.status, 101)
		link.webSocket.accept()
		const introduced = []
		link.webSocket.addEventListener('message', (event) => {
			const introduction = JSON.parse(event.data)
			introduced.push(introduction.dialer)
			link.webSocket.send(
				JSON.stringify({ request: introduction.request, candidates: ['198.51.100.7:16662'] }),
			)
		})

		const answer = await introduce('mra_owner')
		assert.equal(answer.status, 200)
		assert.deepEqual(await answer.json(), { candidates: ['198.51.100.7:16662'] })
		assert.deepEqual(introduced, [dialer])

		// Nobody but the owner is introduced, and the Core is not told about them.
		assert.equal((await introduce('mra_friend')).status, 404)
		assert.equal((await introduce('mra_owner', { id: 'short', candidates: [] })).status, 400)
		assert.equal(
			(await introduce('mra_owner', { ...dialer, candidates: ['core.example:1'] })).status,
			400,
		)
		assert.deepEqual(introduced, [dialer])
		assert.equal(await writes(), before)

		// A key no Core has cannot hold a link.
		const stranger = await backend.fetch('http://local.test/core/link', {
			headers: { upgrade: 'websocket', authorization: 'Bearer amc_other' },
		})
		assert.equal(stranger.status, 410)

		// Removing the Core drops its link, so it learns at once.
		const closed = new Promise((resolve) => link.webSocket.addEventListener('close', resolve))
		assert.equal((await call('DELETE', '/cores/core', undefined, 'mra_owner')).status, 204)
		await closed
	})
})

test('tickets carry the user and the Core, for the owner only', async () => {
	await withCore('core-tickets', async ({ call, writes }) => {
		const before = await writes()

		const response = await call('POST', '/cores/core/ticket', undefined, 'mra_owner')
		assert.equal(response.status, 200)
		const { ticket } = await response.json()
		const { header, claims, valid } = decode(ticket)
		assert.ok(valid, 'the signature verifies with the public key')
		assert.deepEqual(header, { alg: 'EdDSA', typ: 'JWT', kid })
		assert.equal(claims.iss, 'amberite')
		assert.equal(claims.sub, 'owner')
		assert.equal(claims.name, 'owner')
		assert.equal(claims.avatar_url, null)
		assert.equal(claims.aud, 'core')
		assert.equal(claims.exp - claims.iat, 300)
		assert.ok(
			Number.isInteger(claims.iat) && claims.iat > 0 && claims.iat < 2 ** 32,
			'iat is in whole seconds',
		)
		assert.equal(typeof claims.jti, 'string')
		if (process.env.WRITE_TICKET) {
			const key = { ...publicKey.export({ format: 'jwk' }), kid }
			const signed = { keys: { keys: [key] }, ticket, issued_at: claims.iat }
			await writeFile(SIGNED_TICKET, JSON.stringify(signed, null, '\t') + '\n')
		}
		const signed = JSON.parse(await readFile(SIGNED_TICKET, 'utf8'))
		const [, signedClaims] = signed.ticket.split('.')
		const shape = (value) =>
			Object.entries(value).map(([key, field]) => [
				key,
				Array.isArray(field) ? 'array' : field === null ? 'null' : typeof field,
			])
		assert.deepEqual(
			shape(JSON.parse(Buffer.from(signedClaims, 'base64url').toString())),
			shape(claims),
			'Core verifies session-ticket.json: sign a new one with WRITE_TICKET=1',
		)

		const again = await call('POST', '/cores/core/ticket', undefined, 'mra_owner')
		assert.notEqual(decode((await again.json()).ticket).claims.jti, claims.jti)

		assert.equal((await call('POST', '/cores/core/ticket', undefined, 'mra_friend')).status, 404)
		assert.equal((await call('POST', '/cores/nope/ticket', undefined, 'mra_owner')).status, 404)
		assert.equal((await call('POST', '/cores/core/ticket', undefined, 'amc_test')).status, 401)
		assert.equal(await writes(), before)
	})
})

test('state reports write only what changed', async () => {
	await withCore('core-state', async ({ backend, call, writes }) => {
		const state = {
			version: '0.1.0',
			address: 'https://192.168.1.2:16662',
			public_address: null,
			cert_fingerprint: 'a'.repeat(64),
		}
		const core = () => backend.db.prepare("SELECT * FROM cores WHERE id = 'core'").first()

		const before = await writes()
		assert.equal((await call('POST', '/core/state', state)).status, 204)
		assert.equal((await core()).cert_fingerprint, 'a'.repeat(64))
		assert.equal((await core()).version, '0.1.0')
		const first = await writes()
		assert.ok(first > before)

		assert.equal((await call('POST', '/core/state', state)).status, 204)
		assert.equal(await writes(), first, 'the same report again writes nothing')

		const moved = { ...state, public_address: 'https://203.0.113.5:16662' }
		assert.equal((await call('POST', '/core/state', moved)).status, 204)
		assert.equal((await core()).public_address, 'https://203.0.113.5:16662')
		assert.equal(await writes(), first + 1, 'one changed field is one write')

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

test('the old Core routes are gone, and a removed Core is told so on every route it calls', async () => {
	await withCore('core-old-routes', async ({ call }) => {
		for (const [method, path] of [
			['POST', '/core/address'],
			['DELETE', '/core/auth'],
			['POST', '/core/reachability'],
			['PUT', '/core/servers'],
		])
			assert.equal((await call(method, path, {})).status, 404, `${method} ${path}`)

		assert.equal((await call('DELETE', '/cores/core', undefined, 'mra_owner')).status, 204)
		// The key is checked before the body is read, so a body that does not fit is not the answer.
		for (const [method, path] of [
			['POST', '/core/state'],
			['POST', '/core/storage'],
			['DELETE', '/core/players'],
		])
			assert.equal((await call(method, path, {})).status, 410, `${method} ${path}`)
		assert.equal((await call('POST', '/core/storage', {}, 'not_a_key')).status, 401)
	})
})
