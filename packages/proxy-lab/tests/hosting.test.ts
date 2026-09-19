import assert from 'node:assert/strict'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import { WebSocket } from 'ws'

import type { Archon } from '../../api-client/src/modules/archon/types.ts'

import { PERSONAS, SERVER_ID, WORLD_ID } from '../src/fixtures.ts'
import { startProxyLabServer } from '../src/server.ts'

test('hosting routes preserve state and the console reports the current power state', async () => {
	const dataDir = await mkdtemp(join(tmpdir(), 'modrinth-proxy-lab-'))
	const server = await startProxyLabServer({ dataDir, port: 0 })
	const authorization = { Authorization: `Bearer ${PERSONAS.owner.token}` }

	try {
		const listing = await fetchJson<Archon.Servers.v0.ServerGetResponse>(
			`${server.origin}/modrinth/v0/servers`,
			authorization,
		)
		assert.equal(listing.servers[0]?.server_id, SERVER_ID)

		const stop = await fetch(`${server.origin}/modrinth/v0/servers/${SERVER_ID}/power`, {
			method: 'POST',
			headers: { ...authorization, 'content-type': 'application/json' },
			body: JSON.stringify({ action: 'Stop' }),
		})
		assert.equal(stop.status, 204)

		const auth = await fetchJson<Archon.Websocket.v0.WSAuth>(
			`${server.origin}/modrinth/v0/servers/${SERVER_ID}/ws`,
			authorization,
		)
		const socketEvents = await collectSocketEvents(auth.url, auth.token, 3)
		assert.deepEqual(socketEvents[0], { event: 'auth-ok' })
		assert.deepEqual(socketEvents[1], { event: 'install-progress', items: [] })
		assert.deepEqual(socketEvents[2], { event: 'power-state', state: 'stopped' })

		const create = await fetchJson<Archon.BackupsQueue.v1.PostBackupQueueResponse>(
			`${server.origin}/v1/servers/${SERVER_ID}/worlds/${WORLD_ID}/backups-queue`,
			{
				...authorization,
				'content-type': 'application/json',
			},
			{ method: 'POST', body: JSON.stringify({ name: 'Test backup' }) },
		)
		assert.match(create.id, /^proxy-lab-backup-/)

		const queue = await fetchJson<Archon.BackupsQueue.v1.BackupsQueueResponse>(
			`${server.origin}/v1/servers/${SERVER_ID}/worlds/${WORLD_ID}/backups-queue`,
			authorization,
		)
		assert.equal(queue.backups.at(-1)?.name, 'Test backup')

		const unsupported = await fetch(`${server.origin}/v1/not-a-real-route`, {
			headers: authorization,
		})
		assert.equal(unsupported.status, 501)
		assert.equal((await unsupported.json()).error, 'proxy_lab_route_not_implemented')

		await waitForTrace(dataDir)
		const trace = await readFile(join(dataDir, 'trace.jsonl'), 'utf8')
		assert.match(trace, /"authorization":"<redacted>"/)
		assert.doesNotMatch(trace, new RegExp(PERSONAS.owner.token))
	} finally {
		await server.close()
		await rm(dataDir, { recursive: true, force: true })
	}
})

test('fixture identity accepts only local persona tokens', async () => {
	const dataDir = await mkdtemp(join(tmpdir(), 'modrinth-proxy-lab-'))
	const server = await startProxyLabServer({ dataDir, port: 0 })
	try {
		const unauthorized = await fetch(`${server.origin}/v3/user`, {
			headers: { Authorization: 'Bearer not-a-fixture-token' },
		})
		assert.equal(unauthorized.status, 401)

		const user = await fetchJson<{ id: string }>(`${server.origin}/v3/user`, {
			Authorization: PERSONAS.recipient.token,
		})
		assert.equal(user.id, PERSONAS.recipient.user.id)

		const forbiddenPreferences = await fetch(
			`${server.origin}/v3/user/${PERSONAS.owner.user.id}/preferences`,
			{
				method: 'PATCH',
				headers: {
					Authorization: PERSONAS.recipient.token,
					'content-type': 'application/json',
				},
				body: JSON.stringify({ appearance: { theme: 'light' } }),
			},
		)
		assert.equal(forbiddenPreferences.status, 403)

		const friends = await fetch(`${server.origin}/v3/friends`, {
			headers: { Authorization: PERSONAS.recipient.token },
		})
		assert.equal(friends.status, 200)
		assert.deepEqual(await friends.json(), [])

		const signIn = await fetch(`${server.origin}/auth/sign-in?ipver=4&port=12345`)
		const html = await signIn.text()
		assert.match(html, /ProxyLabOwner/)
		assert.match(html, /ProxyLabRecipient/)
	} finally {
		await server.close()
		await rm(dataDir, { recursive: true, force: true })
	}
})

async function fetchJson<T>(
	url: string,
	headers: Record<string, string>,
	init: RequestInit = {},
): Promise<T> {
	const response = await fetch(url, { ...init, headers })
	if (!response.ok) assert.fail(await response.text())
	return (await response.json()) as T
}

function collectSocketEvents(url: string, token: string, count: number): Promise<unknown[]> {
	return new Promise((resolve, reject) => {
		const events: unknown[] = []
		const socket = new WebSocket(url)
		const timeout = setTimeout(() => {
			socket.terminate()
			reject(new Error('Timed out waiting for proxy-lab WebSocket events'))
		}, 2_000)
		socket.on('open', () => socket.send(JSON.stringify({ event: 'auth', jwt: token })))
		socket.on('error', reject)
		socket.on('message', (message) => {
			events.push(JSON.parse(message.toString()) as unknown)
			if (events.length === count) {
				clearTimeout(timeout)
				socket.close()
				resolve(events)
			}
		})
	})
}

async function waitForTrace(dataDir: string): Promise<void> {
	for (let attempt = 0; attempt < 20; attempt++) {
		try {
			await readFile(join(dataDir, 'trace.jsonl'))
			return
		} catch {
			await new Promise((resolve) => setTimeout(resolve, 10))
		}
	}
	throw new Error('Trace file was not written')
}
