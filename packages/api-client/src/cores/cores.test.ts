import { PROTOCOL_VERSION } from '@amberite/contracts/protocol'
import type { ListedCore } from '@amberite/contracts/backend'
import type { Server } from '@amberite/contracts/core'
import { describe, expect, it, vi } from 'vite-plus/test'

import type { AbstractModrinthClient } from '../core/abstract-client'
import { READY_MS } from '../events/event-stream'
import { Cores } from './cores'

import { SelfHostedFeature } from '../features/self-hosted'
import { GenericModrinthClient } from '../platform/generic'
import type { RequestOptions } from '../types/request'

const ACCOUNTS = 'https://accounts.example'
const CORE: ListedCore = {
	id: 'core',
	name: 'Core',
	version: null,
	address: 'https://192.168.1.2:16662',
	public_address: 'https://203.0.113.5:16662',
	cert_fingerprint: 'ab'.repeat(32),
	created: '',
	last_seen: '',
}
const SERVER = {
	server_id: '00000000-0000-4000-8000-000000000001',
	net: { ip: '203.0.113.5', port: 41000, outside: null, game_port: 25565 },
}
const LISTED: Server = {
	server_id: SERVER.server_id,
	name: 'Survival',
	owner_id: 'owner',
	status: 'available',
	loader: null,
	loader_version: null,
	mc_version: null,
	upstream: null,
	flows: { intro: true },
	icon: null,
	net: { ip: SERVER.net.ip, port: SERVER.net.port, outside: null, game_port: 25565 },
	power_state: 'stopped',
	linked_instance: null,
	uploaded_modpack: null,
	shared_instance_id: null,
	reachability: null,
	players: null,
	memory_mb: 2048,
	created_at: '1970-01-01T00:00:00Z',
	update_channel: 'release',
	eula_accepted: true,
}

const DIALER = { id: 'cd'.repeat(32), candidates: ['198.51.100.9:40000'] }
const INTRODUCED = ['203.0.113.5:16662']

/**
 * Lists `CORE`, which has `LISTED`. With `home` the Core answers on this computer's network.
 * Otherwise it is reached once the backend introduced this device, unless it is `off`.
 */
class FakeClient extends GenericModrinthClient {
	/** What `connectCore` was given as the backend's introduction, per call. */
	public connects: (string[] | undefined)[] = []
	public introductions = 0
	public off = false

	constructor(private readonly home: boolean) {
		super({
			labrinthBaseUrl: ACCOUNTS,
			connectCore: async (_coreId, _address, _fingerprint, introduced) => {
				this.connects.push(introduced)
				const base = 'http://127.0.0.1:5000/secret/core/0'
				if (this.home) return { base, reached: 'local', dialer: null }
				if (introduced && !this.off) return { base, reached: 'internet', dialer: null }
				return { base, reached: null, dialer: introduced ? null : DIALER }
			},
			features: [new SelfHostedFeature({ baseUrl: ACCOUNTS, token: async () => undefined })],
		})
	}

	protected async platformFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
		const url = String(input)
		if (url === `${ACCOUNTS}/cores`) return Response.json([CORE])
		if (url === `${ACCOUNTS}/cores/${CORE.id}/introduce`) {
			this.introductions++
			expect(JSON.parse(String(init?.body))).toEqual(DIALER)
			return this.off
				? Response.json({ error: 'core_offline', description: '' }, { status: 503 })
				: Response.json({ candidates: INTRODUCED })
		}
		if (url.endsWith('/health')) return Response.json({ protocol: PROTOCOL_VERSION })
		if (url.endsWith('/servers')) return Response.json([LISTED])
		return new Response(null, { status: 404 })
	}

	protected async executeRequest<T>(_url: string, _options: RequestOptions): Promise<T> {
		return {} as T
	}
}

describe('connecting to a Core', () => {
	it('leaves the backend out when the Core answers on this network', async () => {
		const client = new FakeClient(true)
		await client.cores.list()
		expect(client.introductions).toBe(0)
		expect(client.connects).toEqual([undefined])
	})

	it('has the backend introduce this device to a Core on another network', async () => {
		const client = new FakeClient(false)
		await client.cores.list()
		expect(client.introductions).toBe(1)
		expect(client.connects).toEqual([undefined, INTRODUCED])
		expect(await client.cores.reconnect(CORE.id)).toBe(true)
	})

	it('asks the backend again for a Core that is off only after a wait that doubles', async () => {
		vi.useFakeTimers()
		try {
			const client = new FakeClient(false)
			client.off = true
			await client.cores.list()
			expect(client.introductions).toBe(1)

			// Requests keep failing meanwhile; none of them asks the backend.
			expect(await client.cores.reconnect(CORE.id)).toBe(false)
			await client.cores.list()
			expect(client.introductions).toBe(1)

			await vi.advanceTimersByTimeAsync(15_000)
			expect(await client.cores.reconnect(CORE.id)).toBe(false)
			expect(client.introductions).toBe(2)
			await vi.advanceTimersByTimeAsync(15_000)
			expect(await client.cores.reconnect(CORE.id)).toBe(false)
			expect(client.introductions).toBe(2)

			client.off = false
			await vi.advanceTimersByTimeAsync(15_000)
			expect(await client.cores.reconnect(CORE.id)).toBe(true)
			expect(client.introductions).toBe(3)
		} finally {
			vi.useRealTimers()
		}
	})
})

describe('the Core of a server', () => {
	it('is the listed Core that has it, and none for a server no Core has', async () => {
		const client = new FakeClient(true)
		expect((await client.cores.coreOf(SERVER.server_id)).id).toBe(CORE.id)

		await expect(client.cores.coreOf('00000000-0000-4000-8000-000000000002')).rejects.toMatchObject(
			{ status: 404, code: 'not_found' },
		)
	})
})

describe('joinNet', () => {
	it('joins at the Core’s own address and the game port when the Core is on this network', async () => {
		const client = new FakeClient(true)
		await client.cores.servers()
		expect(client.cores.joinNet(SERVER)).toMatchObject({
			ip: '192.168.1.2',
			port: 25565,
		})
	})

	it('joins where players do when the Core is reached from the internet', async () => {
		const client = new FakeClient(false)
		await client.cores.servers()
		expect(client.cores.joinNet(SERVER)).toBe(SERVER.net)
	})
})

describe('a long op on a Core', () => {
	/**
	 * A Core whose socket hands its events to `handlers` and its status to `status`, and which runs
	 * `running` ops.
	 */
	function core(running: string[]) {
		const handlers = new Map<string, (payload: unknown) => void>()
		let status: (open: boolean) => void = () => {}
		const events = {
			ready: async () => {},
			on: (type: string, handler: (payload: unknown) => void) => {
				handlers.set(type, handler)
				return () => handlers.delete(type)
			},
			onStatus: (handler: (open: boolean) => void) => {
				status = handler
				return () => {}
			},
		}
		const client = {
			coreEvents: () => events,
			core: () => ({ call: async () => running }),
		} as unknown as AbstractModrinthClient
		let started: string | undefined
		const ending = new Cores(client).op('core', async (op) => {
			started = op
		})
		return { handlers, ending, started: () => started, status: (open: boolean) => status(open) }
	}

	it('ends as its end on the socket says', async () => {
		const { handlers, ending, started } = core([])
		await vi.waitFor(() => expect(started()).toBeDefined())
		handlers.get('op_ended')!({ op: 'another', error: null })
		handlers.get('op_ended')!({
			op: started(),
			error: { error: 'not_found', description: 'There is no server.' },
		})
		await expect(ending).rejects.toMatchObject({ code: 'not_found' })
		expect(handlers.size).toBe(0)
	})

	it('ends as lost when a resync lost its end', async () => {
		const { handlers, ending, started } = core(['another'])
		await vi.waitFor(() => expect(started()).toBeDefined())
		handlers.get('resync')!(undefined)
		await expect(ending).rejects.toMatchObject({ code: 'unknown' })
	})

	it('fails once the socket drops and stays down, not while it comes back', async () => {
		vi.useFakeTimers()
		try {
			const { ending, started, status } = core([])
			await vi.waitFor(() => expect(started()).toBeDefined())
			const failed = expect(ending).rejects.toMatchObject({ code: 'network_error' })
			status(false)
			await vi.advanceTimersByTimeAsync(READY_MS - 1)
			status(true)
			status(false)
			await vi.advanceTimersByTimeAsync(READY_MS)
			await failed
		} finally {
			vi.useRealTimers()
		}
	})
})
