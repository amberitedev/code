import { type ErrorCode, PROTOCOL_VERSION } from '@amberite/contracts/protocol'
import type { ListedCore } from '@amberite/contracts/backend'
import { core, type Server } from '@amberite/contracts/core'
import { describe, expect, it } from 'vite-plus/test'

import { ModrinthApiError } from '../core/errors'
import { GenericModrinthClient } from '../platform/generic'
import type { RequestOptions } from '../types/request'
import { AuthFeature } from './auth'
import { ProtocolMismatch, protocolMismatch } from './core-session'
import { SelfHostedFeature } from './self-hosted'

const ACCOUNTS = 'https://accounts.example'
const ACCOUNT = 'mra_account'
/** The local proxy `connectCore` opens for the Core, with its capability path. */
const PROXY = 'http://127.0.0.1:5000/c0ffee'
const SERVER = '00000000-0000-4000-8000-000000000001'
/** `SERVER` as the Core lists it. */
const LISTED: Server = {
	server_id: SERVER,
	name: 'Survival',
	owner_id: 'owner',
	status: 'available',
	loader: null,
	loader_version: null,
	mc_version: null,
	upstream: null,
	flows: { intro: true },
	icon: null,
	net: { ip: null, port: 25565, outside: null, game_port: 25565 },
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
const CORE: ListedCore = {
	id: 'core',
	name: 'Core',
	version: null,
	address: 'https://192.168.1.2:16662',
	public_address: null,
	cert_fingerprint: 'ab'.repeat(32),
	created: '',
	last_seen: '',
}

type Sent = {
	url: string
	method: string
	headers: Record<string, string>
	body?: unknown
	params?: RequestOptions['params']
}

/**
 * The real request pipeline against a fake backend and Core. The backend lists `CORE` and numbers
 * its tickets; the Core answers a GET of a path in `routes` with its entry, and any other request
 * with the next entry of `answers`, or `{}`. Its health reports this App's protocol, and it has
 * `SERVER`.
 */
class FakeClient extends GenericModrinthClient {
	public sent: Sent[] = []
	public answers: unknown[] = []
	public routes: Record<string, unknown> = {
		[`${PROXY}/health`]: { protocol: PROTOCOL_VERSION, version: '0.0.0', core_id: CORE.id },
		[`${PROXY}/servers`]: [LISTED],
	}
	public signIns: string[][] = []
	public hasSession = false
	private tickets = 0

	constructor() {
		super({
			labrinthBaseUrl: ACCOUNTS,
			headers: { Authorization: `Bearer ${ACCOUNT}` },
			connectCore: async (coreId) => (coreId === CORE.id ? PROXY : 'unknown'),
			coreSessions: {
				signIn: async (...args) => {
					this.signIns.push(args)
					this.hasSession = true
					return {} as never
				},
				signOut: async () => {
					this.hasSession = false
				},
				exists: async () => this.hasSession,
			},
			features: [
				new SelfHostedFeature({ baseUrl: ACCOUNTS, token: async () => ACCOUNT }),
				new AuthFeature({ token: ACCOUNT }),
			],
		})
	}

	/** The backend, and the Core as `client.core(id)` reaches it. */
	protected async platformFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
		const url = String(input)
		this.sent.push({
			url,
			method: init?.method ?? 'GET',
			headers: { ...(init?.headers as Record<string, string>) },
			body: typeof init?.body === 'string' ? JSON.parse(init.body) : init?.body,
		})
		if (url === `${ACCOUNTS}/cores`) return Response.json([CORE])
		if (url.endsWith('/ticket')) return Response.json({ ticket: `ticket-${++this.tickets}` })
		if (!url.startsWith(PROXY)) return new Response(null, { status: 204 })
		const answer = this.answer(this.sent.at(-1)!)
		if (answer instanceof ModrinthApiError)
			return Response.json(answer.responseData, { status: answer.statusCode })
		if (answer instanceof Error) throw answer
		return Response.json(answer ?? {})
	}

	protected async executeRequest<T>(url: string, options: RequestOptions): Promise<T> {
		this.sent.push({
			url,
			method: options.method ?? 'GET',
			headers: { ...options.headers },
			body: options.body,
			params: options.params,
		})
		const answer = this.answer(this.sent.at(-1)!)
		if (answer instanceof Error) throw answer
		return (answer ?? {}) as T
	}

	private answer(request: Sent): unknown {
		return this.routed(request) ? this.routes[request.url.split('?')[0]] : this.answers.shift()
	}

	private routed({ url, method }: Sent): boolean {
		return method === 'GET' && url.split('?')[0] in this.routes
	}

	/** What reached the Core, past what `routes` answers: the protocol check and its servers. */
	toCore(): Sent[] {
		return this.sent.filter((request) => request.url.startsWith(PROXY) && !this.routed(request))
	}
}

/** A header by name in any letter case: the self-hosted feature lowercases them. */
function header(headers: Record<string, string>, name: string) {
	return Object.entries(headers).find(([key]) => key.toLowerCase() === name.toLowerCase())?.[1]
}

function coreError(error: ErrorCode) {
	return new ModrinthApiError('HTTP 401', {
		statusCode: 401,
		responseData: { error, description: '' },
	})
}

describe('Core sessions', () => {
	it('signs in with a fresh ticket when this device has no session', async () => {
		const api = new FakeClient()
		await api.cores.ensureSession(CORE)
		expect(api.signIns).toEqual([[CORE.id, CORE.address, CORE.cert_fingerprint, 'ticket-1']])

		await api.cores.ensureSession(CORE)
		expect(api.signIns).toHaveLength(1)
		expect(api.sent.filter((request) => request.url.endsWith('/ticket'))).toHaveLength(1)
	})

	it('signs in once and retries a Core request its session no longer works for', async () => {
		const api = new FakeClient()
		delete api.routes[`${PROXY}/servers`]
		api.answers = [coreError('invalid_credentials'), []]
		await api.cores.servers()
		expect(api.signIns).toHaveLength(1)

		api.answers = [coreError('invalid_credentials'), []]
		expect(await api.core(CORE.id).call(core.ListSessions)).toEqual([])
		expect(api.signIns).toHaveLength(2)
		expect(api.toCore().map((request) => request.url)).toEqual([
			`${PROXY}/servers`,
			`${PROXY}/servers`,
			`${PROXY}/sessions`,
			`${PROXY}/sessions`,
		])
	})

	it('gives up after one sign-in, and never signs in for a refused ticket', async () => {
		const api = new FakeClient()
		api.answers = [coreError('invalid_credentials'), coreError('invalid_credentials')]
		await expect(api.core(CORE.id).call(core.ListSessions)).rejects.toMatchObject({
			statusCode: 401,
		})
		expect(api.signIns).toHaveLength(1)

		api.answers = [coreError('ticket_reused')]
		await expect(
			api.core(CORE.id).call(core.DeleteServer, { params: { id: SERVER } }),
		).rejects.toMatchObject({
			statusCode: 401,
		})
		expect(api.signIns).toHaveLength(1)
	})

	it('sends every dangerous action with a ticket of its own', async () => {
		const api = new FakeClient()
		await api.core(CORE.id).call(core.DeleteServer, { params: { id: SERVER } })
		await api.core(CORE.id).call(core.RevokeSession, { params: { id: 'other' } })
		await api.core(CORE.id).call(core.ListSessions)

		const [deleted, revoked, listed] = api.toCore()
		expect(deleted.url).toBe(`${PROXY}/servers/${SERVER}`)
		expect(header(deleted.headers, 'X-Amberite-Ticket')).toBe('ticket-1')
		expect(header(revoked.headers, 'X-Amberite-Ticket')).toBe('ticket-2')
		expect(header(listed.headers, 'X-Amberite-Ticket')).toBeUndefined()
	})

	it('tells the Core before the backend when a Core is removed', async () => {
		const api = new FakeClient()
		api.hasSession = true
		await api.cores.list()
		const events = api.coreEvents(CORE.id)
		await api.cores.revoke(CORE)
		const calls = api.sent
			.filter((request) => !request.url.endsWith('/ticket'))
			.map((request) => `${request.method} ${request.url}`)
		expect(calls.slice(-2)).toEqual([`DELETE ${PROXY}/pairing`, `DELETE ${ACCOUNTS}/cores/core`])
		expect(api.hasSession).toBe(false)
		// Its socket and routes are dropped with it.
		expect(api.coreEvents(CORE.id)).not.toBe(events)
	})

	it('removes a Core it cannot reach from the backend alone', async () => {
		const api = new FakeClient()
		api.answers = [new ModrinthApiError('Network error')]
		await api.cores.revoke(CORE)
		expect(api.sent.at(-1)?.url).toBe(`${ACCOUNTS}/cores/core`)
	})

	it('never sends the account token to a Core', async () => {
		const api = new FakeClient()
		await api.core(CORE.id).call(core.DeleteServer, { params: { id: SERVER } })
		await api.core(CORE.id).call(core.ListSessions)
		expect(api.toCore()).toHaveLength(2)
		for (const { headers } of api.sent.filter((request) => request.url.startsWith(PROXY)))
			expect(Object.values(headers).some((value) => value.includes(ACCOUNT))).toBe(false)
		const ticket = api.sent.find((request) => request.url.endsWith('/ticket'))
		expect(header(ticket!.headers, 'Authorization')).toBe(`Bearer ${ACCOUNT}`)
	})
})

describe('a Core on another protocol version', () => {
	it('says which side to update', () => {
		expect(protocolMismatch({ protocol: 1 }, 2)?.message).toBe(
			'This Core is out of date. Update this Core.',
		)
		expect(protocolMismatch({ protocol: 3 }, 2)?.message).toBe(
			'This Core is newer than this App. Update Amberite.',
		)
	})

	it('refuses every call but updating the Core, until it is updated', async () => {
		const api = new FakeClient()
		const health = `${PROXY}/health`
		api.routes[health] = { protocol: 0, version: '0.0.0', core_id: CORE.id }
		await expect(api.core(CORE.id).call(core.GetStorage)).rejects.toBeInstanceOf(ProtocolMismatch)
		await api.core(CORE.id).call(core.InstallUpdate, { body: {} })
		expect(api.toCore().map((request) => request.url)).toEqual([`${PROXY}/update/apply`])

		// A failed check runs again; a passed one is shared, even by calls that start together.
		api.routes[health] = { protocol: PROTOCOL_VERSION, version: '0.0.1', core_id: CORE.id }
		const storage = () => api.core(CORE.id).call(core.GetStorage)
		await Promise.all([storage(), storage()])
		await storage()
		expect(api.sent.filter((request) => request.url === health)).toHaveLength(2)
		expect(api.toCore()).toHaveLength(4)
	})
})

describe('creating a server on a Core', () => {
	const EULA = 'https://aka.ms/MinecraftEULA'
	const notAccepted = () =>
		new ModrinthApiError('HTTP 409', {
			statusCode: 409,
			responseData: { error: 'eula_not_accepted', description: '' },
		})
	const toCore = (api: FakeClient) =>
		api.toCore().map((request) => `${request.method} ${request.url.slice(PROXY.length)}`)

	it('asks the owner for the EULA the Core needs first, then creates the server', async () => {
		const api = new FakeClient()
		api.answers = [notAccepted(), { url: EULA, accepted_at: null }, {}, { id: SERVER }]
		const asked: string[] = []
		const created = await api.cores.createServer('Survival', CORE, async (url) => {
			asked.push(url)
			return true
		})
		expect(created).toEqual({ id: SERVER })
		expect(asked).toEqual([EULA])
		expect(toCore(api)).toEqual(['POST /servers', 'GET /eula', 'POST /eula', 'POST /servers'])
	})

	it('creates nothing when the owner declines the EULA', async () => {
		const api = new FakeClient()
		api.answers = [notAccepted(), { url: EULA, accepted_at: null }]
		const created = await api.cores.createServer('Survival', CORE, async () => false)
		expect(created).toBeNull()
		expect(toCore(api)).toEqual(['POST /servers', 'GET /eula'])
	})
})
