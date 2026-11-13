import {
	core,
	type CoreServer,
	type ErrorCode,
	type ListedCore,
	PROTOCOL_VERSION,
} from '@amberite/contracts'
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
const CORE: ListedCore = {
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
 * its tickets; the Core answers a path in `routes` with its entry, and any other request with the
 * next entry of `answers`, or `{}`. Its health reports no protocol, which is version 1.
 */
class FakeClient extends GenericModrinthClient {
	public sent: Sent[] = []
	public answers: unknown[] = []
	public routes: Record<string, unknown> = {
		[`${PROXY}/health`]: { version: '0.0.0', core_id: CORE.id },
	}
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
		const answer = this.answer(url)
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
		const answer = this.answer(url)
		if (answer instanceof Error) throw answer
		return (answer ?? {}) as T
	}

	private answer(url: string): unknown {
		const path = url.split('?')[0]
		return path in this.routes ? this.routes[path] : this.answers.shift()
	}

	/** What reached the Core, past the protocol check. */
	toCore(): Sent[] {
		return this.sent.filter(
			(request) => request.url.startsWith(PROXY) && request.url !== `${PROXY}/health`,
		)
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
		await api.archon.cores_v1.ensureSession(CORE)
		expect(api.signIns).toEqual([[CORE.id, CORE.address, CORE.cert_fingerprint, 'ticket-1']])

		await api.archon.cores_v1.ensureSession(CORE)
		expect(api.signIns).toHaveLength(1)
		expect(api.sent.filter((request) => request.url.endsWith('/ticket'))).toHaveLength(1)
	})

	it('signs in once and retries a Core request its session no longer works for', async () => {
		const api = new FakeClient()
		api.answers = [coreError('invalid_credentials'), []]
		await api.archon.cores_v1.servers()
		expect(api.signIns).toHaveLength(1)

		api.answers = [coreError('invalid_credentials'), { owner: true }]
		expect(await api.core(CORE.id).call(core.GetMyAccess)).toEqual({ owner: true })
		expect(api.signIns).toHaveLength(2)
		expect(api.toCore().map((request) => request.url)).toEqual([
			`${PROXY}/servers`,
			`${PROXY}/servers`,
			`${PROXY}/access/me`,
			`${PROXY}/access/me`,
		])
	})

	it('gives up after one sign-in, and never signs in for a refused ticket', async () => {
		const api = new FakeClient()
		api.answers = [coreError('invalid_credentials'), coreError('invalid_credentials')]
		await expect(api.core(CORE.id).call(core.GetMyAccess)).rejects.toMatchObject({
			statusCode: 401,
		})
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
		await api.core(CORE.id).call(core.InviteMember, {
			params: { id: SERVER },
			body: { user: { id: 'friend', name: 'Friend', avatar_url: null }, role: 'editor' },
		})
		await api.core(CORE.id).call(core.GetMyAccess)

		const [deleted, invited, me] = api.toCore()
		expect(deleted.url).toBe(`${PROXY}/servers/${SERVER}`)
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
		expect(calls.slice(-2)).toEqual([`DELETE ${PROXY}/pairing`, `DELETE ${ACCOUNTS}/cores/core`])
		expect(api.hasSession).toBe(false)
	})

	it('removes a Core it cannot reach from the backend alone', async () => {
		const api = new FakeClient()
		api.answers = [new ModrinthApiError('Network error')]
		await api.archon.cores_v1.revoke(CORE)
		expect(api.sent.at(-1)?.url).toBe(`${ACCOUNTS}/cores/core`)
	})

	it('never sends the account token to a Core', async () => {
		const api = new FakeClient()
		await api.archon.servers_v1.deleteLocal(SERVER)
		await api.core(CORE.id).call(core.GetMyAccess)
		api.answers = [[]]
		await api.archon.cores_v1.servers()
		expect(api.toCore()).toHaveLength(3)
		for (const { headers } of api.sent.filter((request) => request.url.startsWith(PROXY)))
			expect(Object.values(headers).some((value) => value.includes(ACCOUNT))).toBe(false)
		const ticket = api.sent.find((request) => request.url.endsWith('/ticket'))
		expect(header(ticket!.headers, 'Authorization')).toBe(`Bearer ${ACCOUNT}`)
	})
})

describe('a Core on another protocol version', () => {
	it('counts a Core that reports none as version 1', () => {
		expect(protocolMismatch({}, true, 1)).toBeNull()
		expect(protocolMismatch({}, true, 2)?.side).toBe('core')
	})

	it('says who can fix it', () => {
		expect(protocolMismatch({ protocol: 1 }, true, 2)?.message).toBe(
			'This Core is out of date. Update this Core.',
		)
		expect(protocolMismatch({ protocol: 1 }, false, 2)?.message).toBe(
			'This Core is out of date. Ask the owner to update it.',
		)
		expect(protocolMismatch({ protocol: 3 }, true, 2)?.message).toBe(
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

	it('invites, changes roles and removes with a ticket each, and leaves without one', async () => {
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
		expect(api.sent.filter((request) => request.url.endsWith('/ticket'))).toHaveLength(3)
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
		const activity = () =>
			api.sent.findLast((request) => request.url.startsWith(`${PROXY}/activity`))?.url
		expect(activity()).toBe(`${PROXY}/activity?server_id=${SERVER}&limit=2`)

		await api.archon.actions_v1.list(SERVER, { limit: 2, offset: 4 })
		expect(activity()).toBe(`${PROXY}/activity?server_id=${SERVER}&limit=2&before=4`)
	})

	it('gives a member an empty action log', async () => {
		const api = new FakeClient()
		api.routes[`${PROXY}/activity`] = forbidden()
		api.routes[`${PROXY}/servers/${SERVER}/access`] = forbidden()
		expect((await api.archon.actions_v1.list(SERVER)).data).toEqual([])
	})
})

const SERVER_ON_CORE: CoreServer = {
	id: SERVER,
	name: 'A',
	owner_id: 'owner',
	role: null,
	installing: false,
	loader: null,
	loader_version: null,
	game_version: null,
	net: { ip: null, port: 25565, outside: null, game_port: 25565 },
	power: 'offline',
	linked_instance: null,
	reachability: null,
	players: null,
	memory_mb: 2048,
	created_at: 0,
	update_channel: 'release',
	eula_accepted: true,
}

describe('a server on a Core', () => {
	it('shows each role its own permissions', async () => {
		const api = new FakeClient()
		for (const [role, permissions] of [
			[null, 0xffff_ffff_ffff_8000],
			['editor', 0xfe00_0000_0000_0000],
			['viewer', 2 ** 63],
		] as const) {
			api.answers = [{ ...SERVER_ON_CORE, role }]
			const shown = await api.archon.servers_v0.get(SERVER)
			expect(shown.current_user_permissions).toBe(permissions)
		}
	})

	it('names the EULA on its page until the owner accepts it', async () => {
		const api = new FakeClient()
		api.answers = [{ ...SERVER_ON_CORE, eula_accepted: false }, SERVER_ON_CORE]
		const [notice] = (await api.archon.servers_v0.get(SERVER)).notices
		expect(notice.message).toContain('Minecraft EULA')
		expect((await api.archon.servers_v0.get(SERVER)).notices).toEqual([])
	})

	it('is renamed on its Core', async () => {
		const api = new FakeClient()
		await api.archon.servers_v0.updateName(SERVER, 'Survival')
		const [renamed] = api.toCore()
		expect(renamed).toMatchObject({
			url: `${PROXY}/servers/${SERVER}`,
			method: 'PATCH',
			body: { name: 'Survival', update_channel: null },
		})
	})

	it('opens no sync stream, since its changes come over the event socket', async () => {
		const api = new FakeClient()
		await api.archon.sync.safeConnectServer(SERVER)
		expect(api.archon.sync.getStatus(SERVER)).toBeNull()
	})
})

describe('URLs a Core names', () => {
	it('keep the proxy base, with its capability, for the event socket and the files', async () => {
		const api = new FakeClient()
		// Core names the host it was reached at, without the capability.
		api.answers = [{ url: `127.0.0.1:5000/servers/${SERVER}`, token: '' }]
		const socket = await api.archon.servers_v0.getWebSocketAuth(SERVER)
		expect(socket.url).toBe('ws://127.0.0.1:5000/c0ffee/events')

		const files = await api.archon.servers_v0.getFilesystemAuth(SERVER)
		expect(files.url).toBe(`${PROXY}/servers/${SERVER}`)
		await api.kyros.files_v0.downloadFileWithAuth(files, '/server.properties')
		expect(api.sent.at(-1)?.url).toBe(`${PROXY}/servers/${SERVER}/modrinth/v0/fs/download`)
	})
})
