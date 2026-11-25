import { type ListedCore, PROTOCOL_VERSION, type Server } from '@amberite/contracts'
import { describe, expect, it, vi } from 'vite-plus/test'

import type { AbstractModrinthClient } from '../core/abstract-client'
import { Cores } from './cores'

import { SelfHostedFeature } from '../features/self-hosted'
import { GenericModrinthClient } from '../platform/generic'
import type { RequestOptions } from '../types/request'

const ACCOUNTS = 'https://accounts.example'
const LOCAL = 'https://192.168.1.2:16662'
const CORE: ListedCore = {
	id: 'core',
	name: 'Core',
	version: null,
	address: LOCAL,
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
	power_state: 'offline',
	linked_instance: null,
	shared_instance_id: null,
	reachability: null,
	players: null,
	memory_mb: 2048,
	created_at: '1970-01-01T00:00:00Z',
	update_channel: 'release',
	eula_accepted: true,
}

/**
 * Lists `CORE`, which has `LISTED`. Its local address answers the health check when `home` is set.
 */
class FakeClient extends GenericModrinthClient {
	constructor(private readonly home: boolean) {
		super({
			labrinthBaseUrl: ACCOUNTS,
			connectCore: async (_coreId, address) =>
				`http://127.0.0.1:5000/${encodeURIComponent(address)}`,
			features: [new SelfHostedFeature({ baseUrl: ACCOUNTS, token: async () => undefined })],
		})
	}

	protected async platformFetch(input: RequestInfo | URL): Promise<Response> {
		const url = String(input)
		if (url === `${ACCOUNTS}/cores`) return Response.json([CORE])
		if (url.endsWith('/health')) return Response.json({ protocol: PROTOCOL_VERSION })
		if (url.endsWith('/servers')) return Response.json([LISTED])
		return new Response(null, { status: 404 })
	}

	protected async executeRequest<T>(url: string, _options: RequestOptions): Promise<T> {
		if (url.includes(encodeURIComponent(LOCAL)) && !this.home) throw new Error('no answer')
		return {} as T
	}
}

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
	it('joins at the Core’s own address and the game port when the Core answers there', async () => {
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
	/** A Core whose socket hands its events to `handlers`, and which runs `running` ops. */
	function core(running: string[]) {
		const handlers = new Map<string, (payload: unknown) => void>()
		const events = {
			ready: async () => {},
			on: (type: string, handler: (payload: unknown) => void) => {
				handlers.set(type, handler)
				return () => handlers.delete(type)
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
		return { handlers, ending, started: () => started }
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
})
