import { type ListedCore, PROTOCOL_VERSION, type Server } from '@amberite/contracts'
import { describe, expect, it } from 'vite-plus/test'

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
	net: { ip: '203.0.113.5', port: 41000, domain: '', game_port: 25565 },
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
