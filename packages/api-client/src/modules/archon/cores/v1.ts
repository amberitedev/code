import { AbstractModule } from '../../../core/abstract-module'
import { ModrinthApiError } from '../../../core/errors'
import type { RequestOptions } from '../../../types/request'
import type { Archon } from '../types'

const SERVER_ID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i
/** How long a Core's local address may take to answer before its internet address is used. */
const LOCAL_ANSWER_MS = 2500
/** How long the Core list is kept after an unreachable Core last caused it to be fetched again. */
const FAILURE_REFETCH_MS = 60_000

/** What is needed to reach a Core: its addresses and the certificate to trust there. */
type CoreLocation = Pick<Archon.Cores.v1.Core, 'id' | 'address' | 'cert_fingerprint'> &
	Partial<Pick<Archon.Cores.v1.Core, 'public_address'>>

/** Where requests to a Core go (`base`), and the Core address that reaches (`address`). */
type Route = { base: string; address: string }

const APPROVAL_MESSAGES: Record<Archon.Cores.v1.ApprovalFailure, string> = {
	wrong_code: 'Code invalid. Please check your Core and try again.',
	expired: 'That code has expired. Use the new code shown on your Core.',
	too_many_attempts: 'Too many wrong codes. Wait a few minutes and try again.',
	already_signed_in: 'That Core is already signed in to another account.',
	failed: 'Could not add the Core. Please try again.',
}

/** Why a Core could not be added. `message` is ready to show. */
export class CoreApprovalError extends Error {
	constructor(
		public readonly reason: Archon.Cores.v1.ApprovalFailure,
		message = APPROVAL_MESSAGES[reason],
	) {
		super(message)
		this.name = 'CoreApprovalError'
	}
}

/**
 * The Cores the account may reach: the ones signed in to it, and the ones it was invited to manage
 * a server on. The backend keeps the list and each Core's address; hosting calls go to a Core
 * directly, with this device's session there (see `copal.access_v1`). Several Cores are treated as
 * one: their servers form one list, and a request for a server is sent to the Core that has it.
 *
 * A Core signs in on its own: it shows a code, and `approve` here is what binds it to the account.
 * The App never contacts a Core before it is listed.
 */
export class ArchonCoresV1Module extends AbstractModule {
	private cores: Promise<Archon.Cores.v1.Core[]> | null = null
	/** When the list was last dropped because a Core did not answer. See `servers`. */
	private droppedAfterFailureAt = 0
	/** The Cores of the last list. */
	private listed: Archon.Cores.v1.Core[] = []
	/** Core id to where its requests go. See `route`. */
	private routes = new Map<string, Route>()
	/** Server id to the id of the Core that has it. */
	private serverCores = new Map<string, string>()

	public getModuleID(): string {
		return 'archon_cores_v1'
	}

	/**
	 * Fetch the Cores the account may reach from the backend, its own first. A Core whose route
	 * cannot be worked out keeps its plain address, so it does not take the other Cores down with it.
	 */
	public list(): Promise<Archon.Cores.v1.Core[]> {
		const cores = this.client
			.request<Archon.Cores.v1.Core[]>('/cores', { api: 'labrinth', version: 3, method: 'GET' })
			.then(async (list) => {
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
		cores.catch(() => {
			if (this.cores === cores) this.cores = null
		})
		this.cores = cores
		return cores
	}

	/**
	 * Which Core is showing `code`, so the user can check it is theirs before approving. Throws a
	 * `CoreApprovalError` for a wrong or expired code.
	 */
	public async lookup(code: string): Promise<Archon.Cores.v1.PendingCore> {
		try {
			return await this.client.request('/cores/approve/lookup', {
				api: 'labrinth',
				version: 3,
				method: 'POST',
				body: { code },
				retry: false,
			})
		} catch (error) {
			throw approvalError(error)
		}
	}

	/**
	 * Approve the Core showing `code`: it signs in to the account and joins the list. The Core
	 * learns of it on its own; refetch the list to see it.
	 */
	public async approve(code: string): Promise<void> {
		try {
			await this.client.request('/cores/approve', {
				api: 'labrinth',
				version: 3,
				method: 'POST',
				body: { code },
				retry: false,
			})
		} catch (error) {
			throw approvalError(error)
		}
		this.cores = null
	}

	/**
	 * Take a Core off the account. The Core is told first, with a fresh ticket: it ends every
	 * session and shows a new pairing code. Then the backend forgets it and its key stops working.
	 * A Core that cannot be told learns on its next call to the backend, so this works even while
	 * the Core is off. Its servers and files stay.
	 */
	public async revoke(core: Archon.Cores.v1.Core): Promise<void> {
		const access = this.client.copal.access_v1
		await access.unpair(core.id).catch((error) => {
			console.warn(`Core ${core.id} was not told it was removed; it learns from the backend`, error)
		})
		await access.signOut(core.id).catch(() => undefined)
		await this.client
			.request(`/cores/${encodeURIComponent(core.id)}`, {
				api: 'labrinth',
				version: 3,
				method: 'DELETE',
			})
			.catch((error) => {
				// Already gone, which is what was asked for.
				if (!(error instanceof ModrinthApiError && error.statusCode === 404)) throw error
			})
		for (const [serverId, coreId] of this.serverCores) {
			if (coreId === core.id) this.serverCores.delete(serverId)
		}
		this.routes.delete(core.id)
		this.listed = this.listed.filter((listed) => listed.id !== core.id)
		this.cores = null
	}

	/**
	 * Accept or decline an invite to a server on someone else's Core. An invite is already access,
	 * so accepting is signing in and declining is leaving the server. The invite is what made that
	 * Core reachable, so the list is fetched again to find it.
	 */
	public async respondToInvite(
		coreId: string,
		serverId: string,
		action: 'accept' | 'decline',
	): Promise<void> {
		const core = (await this.list()).find((candidate) => candidate.id === coreId)
		if (!core) throw new ModrinthApiError('This invite is no longer available', { statusCode: 404 })
		if (action === 'accept') {
			await this.client.copal.access_v1.ensureSession(core)
			this.serverCores.set(serverId, core.id)
			return
		}
		await this.client.copal.access_v1.leave(core.id, serverId)
		// Declining the only server on that Core takes the Core out of the list.
		this.serverCores.delete(serverId)
		this.cores = null
	}

	/** The listed Core a request URL goes to, if any. */
	public coreForUrl(url: string): Archon.Cores.v1.Core | undefined {
		return this.listed.find((core) => {
			const base = this.routes.get(core.id)?.base
			return base !== undefined && (url === base || url.startsWith(`${base}/`))
		})
	}

	/** The listed Core with this id. */
	public async core(coreId: string): Promise<Archon.Cores.v1.Core> {
		const core = (await this.known()).find((candidate) => candidate.id === coreId)
		if (!core) throw new ModrinthApiError('This Core is not listed', { statusCode: 404 })
		return core
	}

	/** The listed Core that has `serverId`. */
	public async coreOf(serverId: string): Promise<Archon.Cores.v1.Core> {
		const cores = await this.known()
		if (cores.length > 1 && !this.serverCores.has(serverId)) await this.servers()
		const coreId = this.serverCores.get(serverId) ?? cores[0]?.id
		const core = cores.find((candidate) => candidate.id === coreId)
		if (!core) throw new ModrinthApiError('No Core is signed in', { statusCode: 404 })
		return core
	}

	/** The base URL requests to a listed Core go to. */
	public async url(coreId: string): Promise<string> {
		return this.base(await this.core(coreId))
	}

	/** The address of a Core its requests reach: its local or its internet address. */
	public address(core: Archon.Cores.v1.Core): string {
		return this.routes.get(core.id)?.address ?? core.address
	}

	/** The hosting API base URL for a request: the Core that has the server the request names. */
	public async hostingUrl(
		path: string,
		params?: RequestOptions['params'],
		version?: RequestOptions['version'],
	): Promise<string> {
		const cores = await this.known()
		if (!cores.length) throw new ModrinthApiError('No Core is signed in', { statusCode: 404 })
		// Uploads carry the server id in the version segment (`servers/<id>/v1`), not the path.
		const serverId =
			cores.length > 1 &&
			`${version ?? ''} ${path} ${JSON.stringify(params ?? {})}`.match(SERVER_ID)
		if (serverId) {
			if (!this.serverCores.has(serverId[0])) await this.servers()
			const core = cores.find((candidate) => candidate.id === this.serverCores.get(serverId[0]))
			if (core) return this.base(core)
		}
		return this.base(cores[0])
	}

	/**
	 * The servers of every listed Core as one list. A Core that cannot be reached is left out and
	 * named in `offline_cores`; when none can be reached this throws. A Core that did not answer
	 * makes the next call ask the backend for the list again, at most once every
	 * `FAILURE_REFETCH_MS`, so polling while a Core is down does not redo the routing every time.
	 */
	public async servers(
		options?: Archon.Servers.v0.GetServersOptions,
	): Promise<Archon.Servers.v0.ServerGetResponse> {
		const cores = await this.known()
		const results = await Promise.allSettled(
			cores.map((core) =>
				this.client.request<Archon.Servers.v0.ServerGetResponse>('/servers', {
					api: this.base(core),
					version: 'modrinth/v0',
					method: 'GET',
					params: { limit: options?.limit ?? 100, offset: options?.offset ?? 0 },
				}),
			),
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
			for (const server of result.value.servers) {
				this.serverCores.set(server.server_id, cores[index].id)
				merged.servers.push(server)
			}
			Object.assign(merged.users, result.value.users)
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
	 * Create an unconfigured server on a Core. Pass `instance` when it is created from an App
	 * instance: the server is linked right away and waits for the instance to be uploaded.
	 */
	public async createServer(
		name: string,
		core: Archon.Cores.v1.Core,
		instance?: {
			id: string
			name: string
			visible_to_friends?: boolean
			shared_instance_id?: string | null
		},
	): Promise<{ id: string }> {
		const server = await this.client.request<{ id: string }>('/servers', {
			api: this.base(core),
			version: 'local',
			method: 'POST',
			body: { name, instance },
		})
		this.serverCores.set(server.id, core.id)
		return server
	}

	private known(): Promise<Archon.Cores.v1.Core[]> {
		return this.cores ?? this.list()
	}

	private base(core: Archon.Cores.v1.Core): string {
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

function approvalError(error: unknown): CoreApprovalError {
	const status = error instanceof ModrinthApiError ? error.statusCode : undefined
	if (status === 404) return new CoreApprovalError('wrong_code')
	if (status === 410) return new CoreApprovalError('expired')
	if (status === 429) return new CoreApprovalError('too_many_attempts')
	if (status === 409) return new CoreApprovalError('already_signed_in')
	if (status === 401) return new CoreApprovalError('failed', 'Sign in to add a Core.')
	if (status === undefined)
		return new CoreApprovalError('failed', 'Could not reach Amberite. Check your connection.')
	return new CoreApprovalError('failed')
}
