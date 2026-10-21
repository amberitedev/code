import { describe, expect, it } from 'vite-plus/test'

import { AuthFeature } from '../../../features/auth'
import { SelfHostedFeature } from '../../../features/self-hosted'
import { ModrinthApiError } from '../../../core/errors'
import { GenericModrinthClient } from '../../../platform/generic'
import type { RequestOptions } from '../../../types/request'
import type { Archon } from '../../archon/types'

const ACCOUNTS = 'https://accounts.example'
const ACCOUNT = 'mra_account'
/** The local proxy `connectCore` opens for the Core, with its capability path. */
const PROXY = 'http://127.0.0.1:5000/c0ffee'
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

type Sent = {
	url: string
	method: string
	headers: Record<string, string>
	body?: unknown
	params?: RequestOptions['params']
}

/**
 * The real request pipeline against a fake backend and Core. The backend lists `CORE` and numbers
 * its tickets; the Core answers a URL in `routes` with its entry, and any other request with the
 * next entry of `answers`, or `{}`.
 */
class FakeClient extends GenericModrinthClient {
	public sent: Sent[] = []
	public answers: unknown[] = []
	public routes: Record<string, unknown> = {}
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
		this.sent.push({
			url,
			method: options.method ?? 'GET',
			headers: { ...options.headers },
			body: options.body,
			params: options.params,
		})
		if (url === `${ACCOUNTS}/v3/cores`) return [CORE] as T
		if (url.endsWith('/ticket')) return { ticket: `ticket-${++this.tickets}` } as T
		const answer = url in this.routes ? this.routes[url] : this.answers.shift()
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

const EDITOR_SCOPES =
	'BASE_READ | POWER_ACTIONS | EXEC_COMMANDS | FILES_WRITE | SETUP | BACKUPS | ADVANCED'
const ACCESS = {
	owner: { id: 'owner', name: 'Owner', avatar_url: null },
	members: [
		{
			user: { id: 'friend', name: 'Friend', avatar_url: null },
			role: 'editor',
			added_at: 1_700_000_000,
		},
	],
}

function forbidden() {
	return new ModrinthApiError('HTTP 403', {
		statusCode: 403,
		responseData: { error: 'forbidden', description: '' },
	})
}

describe('the server access page on a Core', () => {
	it('lists the owner and members in the hosting shapes, none of them pending', async () => {
		const api = new FakeClient()
		api.answers = [ACCESS]
		expect(await api.archon.server_users_v1.list(SERVER)).toEqual([
			{
				user: { id: 'owner', username: 'Owner', avatar_url: null },
				added_on: null,
				permissions: 'SERVER_ADMIN',
			},
			{
				user: { id: 'friend', username: 'Friend', avatar_url: null },
				added_on: new Date(1_700_000_000_000).toISOString(),
				permissions: EDITOR_SCOPES,
			},
		])
		expect(api.toCore().map((request) => request.url)).toEqual([
			`${PROXY}/servers/${SERVER}/access`,
		])
	})

	it('shows a member only themselves', async () => {
		const api = new FakeClient()
		api.answers = [
			forbidden(),
			{
				user: ACCESS.members[0].user,
				owner: false,
				servers: [{ server_id: SERVER, role: 'viewer' }],
			},
		]
		expect(await api.archon.server_users_v1.list(SERVER)).toEqual([
			{
				user: { id: 'friend', username: 'Friend', avatar_url: null },
				added_on: null,
				permissions: 'BASE_READ',
			},
		])
	})

	it('invites, changes roles, and removes with a ticket each, and leaves without one', async () => {
		const api = new FakeClient()
		api.answers = [{ id: 'other', username: 'Other', avatar_url: null }]
		const users = api.archon.server_users_v1
		await users.add(SERVER, { user_id: 'other', role: 'Editor' })
		await users.update(SERVER, 'other', 'Viewer')
		await users.delete(SERVER, 'other')
		api.answers = [{ user: ACCESS.members[0].user, owner: false, servers: [] }]
		await users.leave(SERVER)

		const calls = api.toCore().map((request) => ({
			call: `${request.method} ${request.url.slice(PROXY.length)}`,
			ticket: header(request.headers, 'X-Amberite-Ticket'),
			body: request.body,
		}))
		expect(calls).toEqual([
			{
				call: `POST /servers/${SERVER}/access`,
				ticket: 'ticket-1',
				body: { user: { id: 'other', name: 'Other', avatar_url: null }, role: 'editor' },
			},
			{
				call: `PATCH /servers/${SERVER}/access/other`,
				ticket: 'ticket-2',
				body: { role: 'viewer' },
			},
			{ call: `DELETE /servers/${SERVER}/access/other`, ticket: 'ticket-3', body: undefined },
			{ call: 'GET /access/me', ticket: undefined, body: undefined },
			{ call: `DELETE /servers/${SERVER}/access/friend`, ticket: undefined, body: undefined },
		])
	})

	it('declining an invite leaves the server', async () => {
		const api = new FakeClient()
		api.answers = [{ user: ACCESS.members[0].user, owner: false, servers: [] }]
		await api.archon.cores_v1.respondToInvite(CORE.id, SERVER, 'decline')
		expect(api.toCore().at(-1)?.method).toBe('DELETE')
		expect(api.toCore().at(-1)?.url).toBe(`${PROXY}/servers/${SERVER}/access/friend`)
	})

	it("reads a server's activity as its action log", async () => {
		const api = new FakeClient()
		const at = 1_700_000_000
		api.routes[`${PROXY}/activity`] = {
			entries: [
				{
					id: 5,
					at,
					user: ACCESS.owner,
					server_id: SERVER,
					action: { type: 'invited', user_id: 'friend', role: 'editor' },
				},
				{ id: 4, at, user: null, server_id: SERVER, action: { type: 'server_started' } },
			],
			next_before: 4,
		}
		api.routes[`${PROXY}/servers/${SERVER}/access`] = ACCESS
		const log = await api.archon.actions_v1.list(SERVER, { limit: 2 })

		expect(log.next_offset).toBe(4)
		expect(log.data.map((entry) => [entry.actor, entry.action])).toEqual([
			[
				{ type: 'user', user_id: 'owner' },
				{ action: 'user_invited', metadata: { user_id: 'friend', permissions: EDITOR_SCOPES } },
			],
			[{ type: 'user', user_id: 'core-cli' }, { action: 'server_started' }],
		])
		expect(log.users.friend.username).toBe('Friend')
		const activity = api.sent.find((request) => request.url === `${PROXY}/activity`)
		expect(activity?.params).toEqual({ server_id: SERVER, limit: 2, before: undefined })

		await api.archon.actions_v1.list(SERVER, { limit: 2, offset: 4 })
		expect(api.sent.findLast((request) => request.url === `${PROXY}/activity`)?.params).toEqual({
			server_id: SERVER,
			limit: 2,
			before: 4,
		})
	})

	it('gives a member an empty action log', async () => {
		const api = new FakeClient()
		api.routes[`${PROXY}/activity`] = forbidden()
		api.routes[`${PROXY}/servers/${SERVER}/access`] = forbidden()
		expect((await api.archon.actions_v1.list(SERVER)).data).toEqual([])
	})
})

describe('URLs a Core names', () => {
	it('keep the proxy base, with its capability, for the console and the files', async () => {
		const api = new FakeClient()
		// Core names the host it was reached at, without the capability.
		api.answers = [
			{ url: `127.0.0.1:5000/servers/${SERVER}/ws`, token: '' },
			{ url: `127.0.0.1:5000/servers/${SERVER}`, token: '' },
		]
		const socket = await api.archon.servers_v0.getWebSocketAuth(SERVER)
		expect(socket.url).toBe(`ws://127.0.0.1:5000/c0ffee/servers/${SERVER}/ws`)

		const files = await api.archon.servers_v0.getFilesystemAuth(SERVER)
		expect(files.url).toBe(`${PROXY}/servers/${SERVER}`)
		await api.kyros.files_v0.downloadFileWithAuth(files, '/server.properties')
		expect(api.sent.at(-1)?.url).toBe(`${PROXY}/servers/${SERVER}/modrinth/v0/fs/download`)
	})
})
