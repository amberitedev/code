import { describe, expect, it } from 'vite-plus/test'

import { SelfHostedFeature } from '../../../features/self-hosted'
import { GenericModrinthClient } from '../../../platform/generic'
import type { RequestOptions } from '../../../types/request'
import type { Archon } from '../types'

const ACCOUNTS = 'https://accounts.example'
const LOCAL = 'https://192.168.1.2:16662'
const CORE: Archon.Cores.v1.Core = {
	id: 'core',
	name: 'Core',
	owned: true,
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

/** Lists `CORE`; its local address answers the health check when `home` is set. */
class FakeClient extends GenericModrinthClient {
	constructor(private readonly home: boolean) {
		super({
			labrinthBaseUrl: ACCOUNTS,
			selfHostedHosting: true,
			connectCore: async (_coreId, address) =>
				`http://127.0.0.1:5000/${encodeURIComponent(address)}`,
			features: [new SelfHostedFeature({ baseUrl: ACCOUNTS, token: async () => undefined })],
		})
	}

	protected async platformFetch(input: RequestInfo | URL): Promise<Response> {
		if (String(input) === `${ACCOUNTS}/cores`) return Response.json([CORE])
		return new Response(null, { status: 404 })
	}

	protected async executeRequest<T>(url: string, _options: RequestOptions): Promise<T> {
		if (url.includes(encodeURIComponent(LOCAL)) && !this.home) throw new Error('no answer')
		return {} as T
	}
}

describe('joinNet', () => {
	it('joins at the Core’s own address and the game port when the Core answers there', async () => {
		const client = new FakeClient(true)
		await client.archon.cores_v1.list()
		expect(client.archon.cores_v1.joinNet(SERVER)).toMatchObject({
			ip: '192.168.1.2',
			port: 25565,
		})
	})

	it('joins where players do when the Core is reached from the internet', async () => {
		const client = new FakeClient(false)
		await client.archon.cores_v1.list()
		expect(client.archon.cores_v1.joinNet(SERVER)).toBe(SERVER.net)
	})
})
