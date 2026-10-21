import { describe, expect, it } from 'vite-plus/test'

import { AuthFeature } from '../../../features/auth'
import { SelfHostedFeature } from '../../../features/self-hosted'
import { ModrinthApiError } from '../../../core/errors'
import { GenericModrinthClient } from '../../../platform/generic'
import type { RequestOptions } from '../../../types/request'
import type { Archon } from '../../archon/types'

const ACCOUNTS = 'https://accounts.example'
const ACCOUNT = 'mra_account'
/** The local proxy `connectCore` opens for the Core. */
const PROXY = 'http://127.0.0.1:5000'
const SERVER = '00000000-0000-4000-8000-000000000001'
const CORE: Archon.Cores.v1.Core = {
	id: 'core',
	name: 'Core',
	owned: true,
	version: null,
	address: 'https://192.168.1.2:16662',
	public_address: null,
	cert_fingerprint: 'ab'.repeat(32),
	created: '',
	last_seen: '',
}

type Sent = { url: string; method: string; headers: Record<string, string> }

/**
 * The real request pipeline against a fake backend and Core. The backend lists `CORE` and numbers
 * its tickets; the Core answers each request with the next entry of `answers`, or `{}`.
 */
class FakeClient extends GenericModrinthClient {
	public sent: Sent[] = []
	public answers: unknown[] = []
	public signIns: string[][] = []
	public hasSession = false
	private tickets = 0

	constructor() {
		super({
			labrinthBaseUrl: ACCOUNTS,
			selfHostedHosting: true,
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

	protected async executeRequest<T>(url: string, options: RequestOptions): Promise<T> {
		this.sent.push({ url, method: options.method ?? 'GET', headers: { ...options.headers } })
		if (url === `${ACCOUNTS}/v3/cores`) return [CORE] as T
		if (url.endsWith('/ticket')) return { ticket: `ticket-${++this.tickets}` } as T
		const answer = this.answers.shift()
		if (answer instanceof Error) throw answer
		return (answer ?? {}) as T
	}

	/** What reached the Core. */
	toCore(): Sent[] {
		return this.sent.filter((request) => request.url.startsWith(PROXY))
	}
}

/** A header by name in any letter case: the self-hosted feature lowercases them. */
function header(headers: Record<string, string>, name: string) {
	return Object.entries(headers).find(([key]) => key.toLowerCase() === name.toLowerCase())?.[1]
}

function coreError(error: string) {
	return new ModrinthApiError('HTTP 401', {
		statusCode: 401,
		responseData: { error, description: '' },
	})
}

describe('copal access', () => {
	it('signs in with a fresh ticket when this device has no session', async () => {
		const api = new FakeClient()
		await api.copal.access_v1.ensureSession(CORE)
		expect(api.signIns).toEqual([[CORE.id, CORE.address, CORE.cert_fingerprint, 'ticket-1']])

		await api.copal.access_v1.ensureSession(CORE)
		expect(api.signIns).toHaveLength(1)
		expect(api.sent.filter((request) => request.url.endsWith('/ticket'))).toHaveLength(1)
	})

	it('signs in once and retries a Core request its session no longer works for', async () => {
		const api = new FakeClient()
		api.answers = [coreError('unauthorized'), { servers: [], users: {} }]
		await api.archon.cores_v1.servers()
		expect(api.signIns).toHaveLength(1)
		expect(api.toCore().map((request) => request.url)).toEqual([
			`${PROXY}/modrinth/v0/servers`,
			`${PROXY}/modrinth/v0/servers`,
		])
	})

	it('gives up after one sign-in, and never signs in for a refused ticket', async () => {
		const api = new FakeClient()
		api.answers = [coreError('unauthorized'), coreError('unauthorized')]
		await expect(api.copal.access_v1.me(CORE.id)).rejects.toMatchObject({ statusCode: 401 })
		expect(api.signIns).toHaveLength(1)

		api.answers = [coreError('ticket_reused')]
		await expect(api.archon.servers_v1.deleteLocal(SERVER)).rejects.toMatchObject({
			statusCode: 401,
		})
		expect(api.signIns).toHaveLength(1)
	})

	it('sends every dangerous action with a ticket of its own', async () => {
		const api = new FakeClient()
		await api.archon.servers_v1.deleteLocal(SERVER)
		await api.copal.access_v1.invite(CORE.id, SERVER, {
			user: { id: 'friend', name: 'Friend', avatar_url: null },
			role: 'editor',
		})
		await api.copal.access_v1.me(CORE.id)

		const [deleted, invited, me] = api.toCore()
		expect(deleted.url).toBe(`${PROXY}/local/servers/${SERVER}`)
		expect(header(deleted.headers, 'X-Amberite-Ticket')).toBe('ticket-1')
		expect(header(invited.headers, 'X-Amberite-Ticket')).toBe('ticket-2')
		expect(header(me.headers, 'X-Amberite-Ticket')).toBeUndefined()
	})

	it('tells the Core before the backend when a Core is removed', async () => {
		const api = new FakeClient()
		api.hasSession = true
		await api.archon.cores_v1.list()
		await api.archon.cores_v1.revoke(CORE)
		const calls = api.sent
			.filter((request) => !request.url.endsWith('/ticket'))
			.map((request) => `${request.method} ${request.url}`)
		expect(calls.slice(-2)).toEqual([`DELETE ${PROXY}/pairing`, `DELETE ${ACCOUNTS}/v3/cores/core`])
		expect(api.hasSession).toBe(false)
	})

	it('removes a Core it cannot reach from the backend alone', async () => {
		const api = new FakeClient()
		api.answers = [new ModrinthApiError('Network error')]
		await api.archon.cores_v1.revoke(CORE)
		expect(api.sent.at(-1)?.url).toBe(`${ACCOUNTS}/v3/cores/core`)
	})

	it('never sends the account token to a Core', async () => {
		const api = new FakeClient()
		await api.archon.servers_v1.deleteLocal(SERVER)
		await api.copal.access_v1.me(CORE.id)
		api.answers = [{ servers: [], users: {} }]
		await api.archon.cores_v1.servers()
		expect(api.toCore()).toHaveLength(3)
		for (const { headers } of api.toCore())
			expect(Object.values(headers).some((value) => value.includes(ACCOUNT))).toBe(false)
		const ticket = api.sent.find((request) => request.url.endsWith('/ticket'))
		expect(header(ticket!.headers, 'Authorization')).toBe(`Bearer ${ACCOUNT}`)
	})
})
