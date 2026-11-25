import {
	type ApiError,
	type CoreSession,
	core as coreRoutes,
	cores,
	type ListedCore,
} from '@amberite/contracts'

import { ApiCallError } from '../contracts/errors'
import type { AbstractModrinthClient } from '../core/abstract-client'
import { archonServer } from '../core-hosting/servers-v0'
import type { Archon } from '../modules/archon/types'
import type { CoreCallError, CoreSessions } from '../types/client'

/** How long a Core's local address may take to answer before its internet address is used. */
const LOCAL_ANSWER_MS = 2500
/** How long the Core list is kept after an unreachable Core last caused it to be fetched again. */
const FAILURE_REFETCH_MS = 60_000

/** What is needed to reach a Core: its addresses and the certificate to trust there. */
type CoreLocation = Pick<ListedCore, 'id' | 'address' | 'cert_fingerprint'> &
	Partial<Pick<ListedCore, 'public_address'>>

/** Where requests to a Core go (`base`), and the Core address that reaches (`address`). */
type Route = { base: string; address: string }

/**
 * The account's own Cores. The backend keeps the list and each Core's address; hosting calls go to
 * a Core directly, with this device's session there (see `signIn`). Several Cores are treated as
 * one: their servers form one list, and a request for a server is sent to the Core that has it.
 *
 * The App never contacts a Core before it is listed. Fetch the list again with `list` after it
 * changes, such as when a Core is approved.
 */
export class Cores {
	private cores: Promise<ListedCore[]> | null = null
	/** When the list was last dropped because a Core did not answer. See `servers`. */
	private droppedAfterFailureAt = 0
	/** The Cores of the last list. */
	private listed: ListedCore[] = []
	/** Core id to where its requests go. See `route`. */
	private routes = new Map<string, Route>()
	/** Server id to the id of the Core that has it. */
	private serverCores = new Map<string, string>()
	/** Sign-ins in flight, by Core id, so requests that fail together sign in once. */
	private signingIn = new Map<string, Promise<CoreSession>>()

	constructor(private readonly client: AbstractModrinthClient) {}

	/**
	 * Fetch the account's Cores from the backend. A Core whose route cannot be worked out keeps its
	 * plain address, so it does not take the other Cores down with it.
	 */
	public list(): Promise<ListedCore[]> {
		const listing = this.client.backend.call(cores.ListCores).then(async (list) => {
			const settled = await Promise.allSettled(list.map((core) => this.route(core)))
			this.routes = new Map(
				list.map((core, index) => {
					const result = settled[index]
					const plain = { base: core.address, address: core.address }
					return [core.id, result.status === 'fulfilled' ? result.value : plain]
				}),
			)
			this.listed = list
			return list
		})
		listing.catch(() => {
			if (this.cores === listing) this.cores = null
		})
		this.cores = listing
		return listing
	}

	/**
	 * Take a Core off the account. The Core is told first, with a fresh ticket: it ends every
	 * session and shows a new pairing code. Then the backend forgets it and its key stops working.
	 * A Core that cannot be told learns on its next call to the backend, so this works even while
	 * the Core is off. Its servers and files stay.
	 */
	public async revoke(core: ListedCore): Promise<void> {
		await this.client
			.core(core.id)
			.call(coreRoutes.Unpair)
			.catch((error: unknown) => {
				console.warn(
					`Core ${core.id} was not told it was removed; it learns from the backend`,
					error,
				)
			})
		await this.signOut(core.id).catch(() => undefined)
		await this.client.backend
			.call(cores.RemoveCore, { params: { id: core.id } })
			.catch((error: unknown) => {
				// Already gone, which is what was asked for.
				if (!(error instanceof ApiCallError && error.code === 'not_found')) throw error
			})
		for (const [serverId, coreId] of this.serverCores) {
			if (coreId === core.id) this.serverCores.delete(serverId)
		}
		this.routes.delete(core.id)
		this.listed = this.listed.filter((listed) => listed.id !== core.id)
		this.cores = null
	}

	/**
	 * Run a long op on a Core: `start` sends its route with a new op id, which the Core answers
	 * at once. Resolves once the op ends on the Core's event socket, and rejects with its error. A
	 * resync may have lost the end: an op the Core no longer runs then ends as lost.
	 */
	public async op(coreId: string, start: (op: string) => Promise<unknown>): Promise<void> {
		const op = crypto.randomUUID()
		const events = this.client.coreEvents(coreId)
		let started = false
		let finish: (error: ApiError | null) => void = () => {}
		const ended = new Promise<void>((resolve, reject) => {
			finish = (error) =>
				error ? reject(new ApiCallError(0, error.error, error.description)) : resolve()
		})
		const stops = [
			events.on('op_ended', (end) => {
				if (end.op === op) finish(end.error)
			}),
			events.on('resync', async () => {
				if (!started) return
				const running = await this.client
					.core(coreId)
					.call(coreRoutes.ListOps)
					.catch(() => null)
				if (running && !running.includes(op))
					finish({ error: 'unknown', description: 'Lost track of the operation' })
			}),
		]
		try {
			// Open first, so the end cannot pass before anyone listens.
			await events.ready()
			await start(op)
			started = true
			await ended
		} finally {
			for (const stop of stops) stop()
		}
	}

	/** A ticket for `coreId`, which only its owner gets. */
	public async ticket(coreId: string): Promise<string> {
		const { ticket } = await this.client.backend.call(cores.IssueTicket, { params: { id: coreId } })
		return ticket
	}

	/** Sign in to `core` unless this device already holds a session there. */
	public async ensureSession(core: ListedCore): Promise<void> {
		if (await this.sessions().exists(core.id)) return
		await this.signIn(core)
	}

	/**
	 * Sign this device in to `core` with a fresh ticket, replacing any session it held. The
	 * platform trades the ticket with the Core for a session of its own and adds it to every
	 * request to the Core; the page never sees it. Throws an `ApiCallError` with Core's error,
	 * such as `clock_wrong`.
	 */
	public signIn(core: ListedCore): Promise<CoreSession> {
		const pending = this.signingIn.get(core.id)
		if (pending) return pending
		const signing = (async () => {
			const fingerprint = core.cert_fingerprint
			if (!fingerprint)
				throw new ApiCallError(400, 'unsupported', 'This Core does not serve HTTPS yet')
			const ticket = await this.ticket(core.id)
			return this.sessions()
				.signIn(core.id, this.address(core), fingerprint, ticket)
				.catch((error: unknown) => {
					throw coreError(error)
				})
		})().finally(() => this.signingIn.delete(core.id))
		this.signingIn.set(core.id, signing)
		return signing
	}

	/** Sign this device out of the Core, and forget its session even when the Core is off. */
	public signOut(coreId: string): Promise<void> {
		return this.sessions().signOut(coreId)
	}

	/** The listed Core a request URL goes to, if any. */
	public coreForUrl(url: string): ListedCore | undefined {
		return this.listed.find((core) => {
			const base = this.routes.get(core.id)?.base
			return base !== undefined && (url === base || url.startsWith(`${base}/`))
		})
	}

	/** The listed Core with this id. */
	public async core(coreId: string): Promise<ListedCore> {
		const core = (await this.known()).find((candidate) => candidate.id === coreId)
		if (!core) throw new ApiCallError(404, 'not_found', 'This Core is not listed')
		return core
	}

	/** The listed Core that has `serverId`. Throws `not_found` when no Core that answers has it. */
	public async coreOf(serverId: string): Promise<ListedCore> {
		if (!this.serverCores.has(serverId)) await this.servers()
		const coreId = this.serverCores.get(serverId)
		const core = (await this.known()).find((candidate) => candidate.id === coreId)
		if (!core) throw new ApiCallError(404, 'not_found', 'No listed Core has this server')
		return core
	}

	/** The base URL requests to a listed Core go to. */
	public async url(coreId: string): Promise<string> {
		return this.base(await this.core(coreId))
	}

	/** The address of a Core its requests reach: its local or its internet address. */
	public address(core: ListedCore): string {
		return this.routes.get(core.id)?.address ?? core.address
	}

	/**
	 * Where this App joins a server. When its Core answers at the Core's own network address, the
	 * App is on that network (or the Core has no router in front of it), so it joins at that address
	 * and the server's game port: owners at home then do not depend on their router taking its own
	 * internet address back in (NAT loopback). Otherwise it is where players join from the internet.
	 * Pass the result to `getHostingServerAddress`; show and copy the plain `net`, which is what
	 * everyone else joins.
	 */
	public joinNet(server: Pick<Archon.Servers.v0.Server, 'server_id' | 'net'>) {
		const { net } = server
		if (net.game_port === undefined) return net
		const coreId = this.serverCores.get(server.server_id)
		const core = this.listed.find((candidate) => candidate.id === coreId)
		if (!core || this.address(core) !== core.address) return net
		const ip = new URL(core.address).hostname.replace(/^\[|\]$/g, '')
		return { ...net, ip, port: net.game_port }
	}

	/** The base URL requests to the Core that has `serverId` go to. */
	public async serverUrl(serverId: string): Promise<string> {
		return this.base(await this.coreOf(serverId))
	}

	/**
	 * The servers of every listed Core as one list. A Core that cannot be reached is left out and
	 * named in `offline_cores`; when none can be reached this throws. A Core that did not answer
	 * makes the next call ask the backend for the list again, at most once every
	 * `FAILURE_REFETCH_MS`, so polling while a Core is down does not redo the routing every time.
	 */
	public async servers(): Promise<Archon.Servers.v0.ServerGetResponse> {
		const cores = await this.known()
		const results = await Promise.allSettled(
			cores.map((core) => this.client.core(core.id).call(coreRoutes.ListServers)),
		)
		const merged: Archon.Servers.v0.ServerGetResponse = {
			servers: [],
			pagination: { current_page: 1, page_size: 0, total_pages: 1, total_items: 0 },
			users: {},
			offline_cores: [],
		}
		results.forEach((result, index) => {
			if (result.status !== 'fulfilled') {
				merged.offline_cores?.push(cores[index].id)
				return
			}
			for (const server of result.value) {
				this.serverCores.set(server.server_id, cores[index].id)
				merged.servers.push(archonServer(server, this.base(cores[index])))
			}
		})
		const failure = results.find((result) => result.status === 'rejected')
		// A Core that did not answer may have moved or changed its certificate: ask the backend again.
		if (failure && Date.now() - this.droppedAfterFailureAt >= FAILURE_REFETCH_MS) {
			this.droppedAfterFailureAt = Date.now()
			this.cores = null
		}
		if (failure && !results.some((result) => result.status === 'fulfilled')) throw failure.reason
		merged.pagination.page_size = merged.pagination.total_items = merged.servers.length
		return merged
	}

	/**
	 * Create an empty server on a Core. A Core makes no server until its owner accepts the Minecraft
	 * EULA, so the first one shows it with `confirmEula`. Null when the owner declines.
	 */
	public async createServer(
		name: string,
		core: ListedCore,
		confirmEula: (url: string) => Promise<boolean>,
	): Promise<{ id: string } | null> {
		const routes = this.client.core(core.id)
		const create = () => routes.call(coreRoutes.CreateServer, { body: { name } })
		const server = await create().catch(async (error: unknown) => {
			if (!(error instanceof ApiCallError && error.code === 'eula_not_accepted')) throw error
			if (!(await confirmEula((await routes.call(coreRoutes.GetEula)).url))) return null
			await routes.call(coreRoutes.AcceptEula)
			return create()
		})
		if (server) this.serverCores.set(server.id, core.id)
		return server
	}

	private sessions(): CoreSessions {
		const sessions = this.client.coreSessions
		if (!sessions) throw new Error('This platform cannot sign in to a Core')
		return sessions
	}

	private known(): Promise<ListedCore[]> {
		return this.cores ?? this.list()
	}

	private base(core: ListedCore): string {
		return this.routes.get(core.id)?.base ?? core.address
	}

	/**
	 * Work out where requests to a Core go. Its local address is used when it answers, which is the
	 * case at home; otherwise its address on the internet. An HTTPS address goes through
	 * `connectCore`, which trusts only that Core's certificate and signs requests with this
	 * device's session there.
	 */
	private async route(core: CoreLocation): Promise<Route> {
		const connect = this.client.connectCore
		const open = async (address: string): Promise<Route> => ({
			address,
			base:
				connect && core.cert_fingerprint && address.startsWith('https:')
					? await connect(core.id, address, core.cert_fingerprint)
					: address,
		})
		const local = await open(core.address)
		if (!core.public_address) return local
		const answered = await Promise.race([
			this.client
				.request('', {
					api: local.base,
					version: 'health',
					method: 'GET',
					skipAuth: true,
					retry: false,
					circuitBreaker: false,
				})
				.then(
					() => true,
					() => false,
				),
			new Promise<false>((resolve) => setTimeout(() => resolve(false), LOCAL_ANSWER_MS)),
		])
		return answered ? local : open(core.public_address)
	}
}

/** A platform sign-in failure as the error a Core call would have thrown. */
function coreError(error: unknown): unknown {
	return isCoreCallError(error)
		? new ApiCallError(error.status ?? 0, error.error, error.description)
		: error
}

function isCoreCallError(error: unknown): error is CoreCallError {
	return (
		typeof error === 'object' &&
		error !== null &&
		'description' in error &&
		typeof error.description === 'string' &&
		'error' in error &&
		typeof error.error === 'string'
	)
}
