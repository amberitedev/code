import { afterEach, describe, expect, it, vi } from 'vite-plus/test'

import type { AbstractModrinthClient } from '../core/abstract-client'
import type { Archon } from '../modules/archon/types'
import { CoreSyncClient, CoreWebSocketClient } from './sockets'

const SERVER = 'server'

class FakeSocket {
	static OPEN = 1
	static last: FakeSocket
	readyState = FakeSocket.OPEN
	sent: unknown[] = []
	onopen = () => {}
	onmessage: (message: { data: string }) => void = () => {}
	onclose: (event: { code: number }) => void = () => {}
	onerror = () => {}

	constructor() {
		FakeSocket.last = this
	}

	send(text: string) {
		this.sent.push(JSON.parse(text))
	}

	close() {}

	receive(event: unknown) {
		this.onmessage({ data: JSON.stringify(event) })
	}
}

/**
 * Connects to `SERVER`, feeds the Core's answers in order, and returns what the panel heard on the
 * socket and on the sync stream.
 */
async function panelHears(answers: unknown[]) {
	vi.stubGlobal('WebSocket', FakeSocket)
	const sync = new CoreSyncClient({} as AbstractModrinthClient)
	const synced: Archon.Sync.v1.SyncEvent[] = []
	sync.onAny(SERVER, (event) => synced.push(event))
	const client = new CoreWebSocketClient({ archon: { sync } } as unknown as AbstractModrinthClient)
	const heard: Archon.Websocket.v0.WSEvent[] = []
	for (const type of ['auth-ok', 'install-progress', 'installation-result'] as const) {
		client.on(SERVER, type, (event) => heard.push(event))
	}
	const connected = client.connect(SERVER, { url: 'ws://core/events', token: '' })
	const socket = FakeSocket.last
	socket.onopen()
	for (const answer of answers) socket.receive(answer)
	await connected
	return { heard, subscribed: socket.sent, synced }
}

const power = { type: 'power', payload: { server: SERVER, state: 'stopped', uptime: null } }

describe('a Core server socket', () => {
	afterEach(() => {
		vi.unstubAllGlobals()
	})

	it('answers the install progress the panel waits for when nothing installs', async () => {
		const { heard, subscribed } = await panelHears([power])
		expect(subscribed[0]).toEqual({ type: 'subscribe', topic: { type: 'installs' } })
		expect(heard).toEqual([{ event: 'auth-ok' }, { event: 'install-progress', items: [] }])
	})

	it('starts with the jobs that were running before the socket authenticated', async () => {
		const job = {
			type: 'install_job',
			payload: {
				job_id: 'job',
				instance_id: SERVER,
				status: 'running',
				progress: { current: 1, total: 4 },
				display: { title: 'Pack' },
				error: null,
			},
		}
		const { heard } = await panelHears([job, power])
		expect(heard).toEqual([
			{ event: 'auth-ok' },
			{
				event: 'install-progress',
				items: [
					{
						world_id: SERVER,
						key: { type: 'local_modpack', filename: 'Pack' },
						id: 'job',
						progress: 25,
						error: null,
					},
				],
			},
		])
	})

	it('has the content and the server fetched again once the server changes it', async () => {
		const edited = { type: 'instance', payload: { instance_id: SERVER, event: 'edited' } }
		const failed = {
			type: 'instance',
			payload: {
				instance_id: SERVER,
				event: 'content_install_failed',
				project_ids: ['p'],
				message: 'no room',
			},
		}
		const { heard, subscribed, synced } = await panelHears([power, edited, failed])
		expect(subscribed).toContainEqual({
			type: 'subscribe',
			topic: { type: 'instance', id: SERVER },
		})
		expect(heard.slice(2)).toEqual([
			{ event: 'installation-result', result: 'ok' },
			{ event: 'installation-result', result: 'err', reason: 'no room' },
		])
		const details = { type: 'server.transfer.done', target_node: '' }
		expect(synced).toEqual([details, details])
	})
})
