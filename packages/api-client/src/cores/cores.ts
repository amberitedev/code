import type { CoreRoute } from '@amberite/contracts/app'
import type { Endpoint } from '@amberite/contracts/protocol'
import { cores, type ListedCore } from '@amberite/contracts/backend'
import { type CoreSession, core as coreRoutes, type Server } from '@amberite/contracts/core'

import type { ContractClient, LocationOf } from '../contracts/client'
import { ApiCallError } from '../contracts/errors'
import type { AbstractModrinthClient } from '../core/abstract-client'
import { READY_MS } from '../events/event-stream'
import type { CoreCallError, CoreSessions } from '../types/client'

/** How long the Core list is kept after an unreachable Core last caused it to be fetched again. */
const FAILURE_REFETCH_MS = 60_000
/**
 * How long a Core the backend could not introduce this device to is left alone: this long after
 * the first failure and twice as long after each next one, up to `MAX_INTRODUCE_WAIT_MS`. Every
 * try is a request the backend pays for, and an App left open next to a Core that is off would
 * keep asking.
 */
const FIRST_INTRODUCE_WAIT_MS = 15_000
const MAX_INTRODUCE_WAIT_MS = 10 * 60_000

/** What is needed to reach a Core: its own address and the certificate to trust there. */
type CoreLocation = Pick<ListedCore, 'id' | 'address' | 'cert_fingerprint'>

/** Where requests to a Core go (`base`), and how the Core is reached, null while it is not. */
type Route = Pick<CoreRoute, 'base' | 'reached'>

/**
 * The account's own Cores. The backend keeps the list and introduces this device to each Core;
 * hosting calls then go to a Core directly, with this device's session there (see `signIn`).
 * Several Cores are treated as one: their servers form one list, and a request for a server is
 * sent to the Core that has it.
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
	/** Core id to its connection started by the last list, settled once it is made or failed. */
	private routing = new Map<string, Promise<void>>()
	/** Server id to the id of the Core that has it. */
	private serverCores = new Map<string, string>()
	/** Sign-ins in flight, by Core id, so requests that fail together sign in once. */
	private signingIn = new Map<string, Promise<CoreSession>>()
	/** Connections being made, by Core id, so requests that fail together connect once. */
	private connecting = new Map<string, Promise<Route>>()
	/** Cores the backend could not introduce: how often in a row, and when it is asked again. */
	private unreached = new Map<string, { failures: number; retryAt: number }>()

	constructor(private readonly client: AbstractModrinthClient) {}

	/**
	 * Fetch the account's Cores from the backend and start connecting to each. Requests to a Core
	 * wait for its own connection only, so one that takes long holds up nobody else.
	 */
	public list(): Promise<ListedCore[]> {
		const listing = this.client.backend.call(cores.ListCores).then((list) => {
			this.routing = new Map(
				list.map((core) => [
					core.id,
					this.route(core).then(
						(route) => {
							this.routes.set(core.id, route)
						},
						() => undefined,
					),
				]),
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
		this.routing.delete(core.id)
		this.unreached.delete(core.id)
		this.listed = this.listed.filter((listed) => listed.id !== core.id)
		this.cores = null
		this.client.forgetCore(core.id)
	}

	/**
	 * Run a long op on a Core: `start` sends its route with a new op id, which the Core answers
	 * at once. Resolves once the op ends on the Core's event socket, and rejects with its error.
	 * Rejects with `network_error`, without starting, when the socket cannot open, and once
	 * started, when the socket drops and stays down for `READY_MS`. A resync may have lost the end:
	 * an op the Core no longer runs then ends as lost.
	 */
	public async op(coreId: string, start: (op: string) => Promise<unknown>): Promise<void> {
		const op = crypto.randomUUID()
		const events = this.client.coreEvents(coreId)
		let started = false
		let finish: (error: ApiCallError | null) => void = () => {}
		const ended = new Promise<void>((resolve, reject) => {
			finish = (error) => (error ? reject(error) : resolve())
		})
		let down: ReturnType<typeof setTimeout> | undefined
		const stops = [
			events.on('op_ended', (end) => {
				if (end.op !== op) return
				const { error } = end
				finish(error && new ApiCallError(0, error.error, error.description))
			}),
			events.on('resync', async () => {
				if (!started) return
				const running = await this.client
					.core(coreId)
					.call(coreRoutes.ListOps)
					.catch(() => null)
				if (running && !running.includes(op))
					finish(new ApiCallError(0, 'unknown', 'Lost track of the operation'))
			}),
			events.onStatus((open) => {
				clearTimeout(down)
				if (open) return
				down = setTimeout(
					() => finish(new ApiCallError(0, 'network_error', 'The Core went offline')),
					READY_MS,
				)
			}),
		]
		try {
			// Open first, so the end cannot pass before anyone listens.
			await events.ready()
			await start(op)
			started = true
			await ended
		} finally {
			clearTimeout(down)
			for (const stop of stops) stop()
		}
	}

	/**
	 * The URL of a route of a Core to download from, such as a backup, for a save that cannot send
	 * the session itself: the App's Core proxy adds it. A call first signs in again when the
	 * session ended.
	 */
	public async downloadUrl<E extends Endpoint>(
		coreId: string,
		endpoint: E,
		location: LocationOf<E>,
	): Promise<string> {
		const core = this.client.core(coreId)
		await core.call(coreRoutes.ListOps)
		return (core.url as (endpoint: E, location: LocationOf<E>) => string)(endpoint, location)
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
			const signIn = () =>
				this.sessions()
					.signIn(core.id, fingerprint, ticket)
					.catch((error: unknown) => {
						throw coreError(error)
					})
			return signIn().catch(async (error: unknown) => {
				// Nothing reached a Core the App had no connection to, so the ticket is unused.
				const offline = error instanceof ApiCallError && error.code === 'core_offline'
				if (offline && (await this.reconnect(core.id))) return signIn()
				throw error
			})
		})().finally(() => this.signingIn.delete(core.id))
		this.signingIn.set(core.id, signing)
		return signing
	}

	/** Sign this device out of the Core, and forget its session even when the Core is off. */
	public signOut(coreId: string): Promise<void> {
		return this.sessions().signOut(coreId)
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

	/** The routes of the Core that has `serverId`. Throws `not_found` when no listed Core has it. */
	public async serverRoutes(serverId: string): Promise<ContractClient> {
		return this.client.core((await this.coreOf(serverId)).id)
	}

	/** The base URL requests to a listed Core go to. */
	public async url(coreId: string): Promise<string> {
		const core = await this.core(coreId)
		await this.routing.get(core.id)
		return this.routes.get(core.id)?.base ?? core.address
	}

	/**
	 * Connect again to a Core whose requests are answered `core_offline`: nothing was sent, so the
	 * caller sends again when this is true. The backend is asked again only after a wait (see
	 * `connect`).
	 */
	public async reconnect(coreId: string): Promise<boolean> {
		const route = await this.route(await this.core(coreId))
		this.routes.set(coreId, route)
		return route.reached !== null
	}

	/**
	 * Where this App joins a server. When its Core is reached on this computer's own network, the
	 * App joins at the Core's address there and the server's game port: owners at home then do not
	 * depend on their router taking its own internet address back in (NAT loopback). Otherwise it
	 * is where players join from the internet. Pass the result to `getHostingServerAddress`; show
	 * and copy the plain `net`, which is what everyone else joins.
	 */
	public joinNet(server: Pick<Server, 'server_id' | 'net'>) {
		const { net } = server
		const coreId = this.serverCores.get(server.server_id)
		const core = this.listed.find((candidate) => candidate.id === coreId)
		if (!core || this.routes.get(core.id)?.reached !== 'local') return net
		const ip = new URL(core.address).hostname.replace(/^\[|\]$/g, '')
		return { ...net, ip, port: net.game_port }
	}

	/**
	 * The servers of every listed Core as one list. A Core that cannot be reached is left out and
	 * named in `offline_cores`; when none can be reached this throws. A Core that did not answer
	 * makes the next call ask the backend for the list again, at most once every
	 * `FAILURE_REFETCH_MS`, so a list read while a Core is down does not redo the routing every time.
	 */
	public async servers(): Promise<{ servers: Server[]; offline_cores: string[] }> {
		const cores = await this.known()
		const results = await Promise.allSettled(
			cores.map((core) => this.client.core(core.id).call(coreRoutes.ListServers)),
		)
		const merged = { servers: [] as Server[], offline_cores: [] as string[] }
		results.forEach((result, index) => {
			if (result.status !== 'fulfilled') {
				merged.offline_cores.push(cores[index].id)
				return
			}
			for (const server of result.value) {
				this.serverCores.set(server.server_id, cores[index].id)
				merged.servers.push(server)
			}
		})
		const failure = results.find((result) => result.status === 'rejected')
		// A Core that did not answer may have changed its certificate: ask the backend again.
		if (failure && Date.now() - this.droppedAfterFailureAt >= FAILURE_REFETCH_MS) {
			this.droppedAfterFailureAt = Date.now()
			this.cores = null
		}
		if (failure && !results.some((result) => result.status === 'fulfilled')) throw failure.reason
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

	/** Connect to a Core and answer where its requests go. Calls made together connect once. */
	private route(core: CoreLocation): Promise<Route> {
		const pending = this.connecting.get(core.id)
		if (pending) return pending
		const connecting = this.connect(core).finally(() => this.connecting.delete(core.id))
		this.connecting.set(core.id, connecting)
		return connecting
	}

	/**
	 * Connect to a Core through `connectCore`, which trusts only that Core's certificate and signs
	 * requests with this device's session there. A Core on this computer's network answers at its
	 * own address, with the backend left out. Any other answers only devices the backend introduced,
	 * so the platform says what to introduce, the backend passes it to the Core and answers with
	 * where the Core is, and the platform connects there. Without `connectCore` a Core is reached
	 * at its address directly.
	 */
	private async connect(core: CoreLocation): Promise<Route> {
		const connect = this.client.connectCore
		if (!connect || !core.cert_fingerprint) return { base: core.address, reached: null }
		let route = await connect(core.id, core.address, core.cert_fingerprint)
		const waiting = this.unreached.get(core.id)
		// Only the backend's part waits: dialing a Core on this network costs it nothing.
		const mayAsk = !waiting || Date.now() >= waiting.retryAt
		const dialer = !route.reached && mayAsk ? route.dialer : null
		if (dialer) {
			const listener = await this.client.backend
				.call(cores.Introduce, { params: { id: core.id }, body: dialer })
				.catch(() => null)
			if (listener)
				route = await connect(core.id, core.address, core.cert_fingerprint, listener.candidates)
		}
		if (route.reached) this.unreached.delete(core.id)
		else if (dialer) {
			const failures = (waiting?.failures ?? 0) + 1
			const wait = Math.min(FIRST_INTRODUCE_WAIT_MS * 2 ** (failures - 1), MAX_INTRODUCE_WAIT_MS)
			this.unreached.set(core.id, { failures, retryAt: Date.now() + wait })
		}
		return { base: route.base, reached: route.reached }
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
