import { AbstractModule } from '../../../core/abstract-module'
import { ModrinthApiError } from '../../../core/errors'
import type { RequestOptions } from '../../../types/request'
import type { Archon } from '../types'

const SERVER_ID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i
/** How long a Core's local address may take to answer before its internet address is used. */
const LOCAL_ANSWER_MS = 2500

/** What is needed to reach a Core: its addresses and the certificate to trust there. */
type CoreLocation = Pick<Archon.Cores.v1.Core, 'address' | 'cert_fingerprint'> &
	Partial<Pick<Archon.Cores.v1.Core, 'public_address'>>

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
 * directly. Several Cores are treated as one: their servers form one list, and a request for a
 * server is sent to the Core that has it.
 *
 * A Core signs in on its own: it shows a code, and `approve` here is what binds it to the account.
 * The App never contacts a Core before it is listed.
 */
export class ArchonCoresV1Module extends AbstractModule {
	private cores: Promise<Archon.Cores.v1.Core[]> | null = null
	/** Base URLs that may receive the account token: the listed Cores. */
	private addresses = new Set<string>()
	/** Core id to the base URL its requests go to. See `route`. */
	private routes = new Map<string, string>()
	/** Server id to the address of the Core that has it. */
	private serverAddresses = new Map<string, string>()

	public getModuleID(): string {
		return 'archon_cores_v1'
	}

	/** Fetch the Cores the account may reach from the backend, its own first. */
	public list(): Promise<Archon.Cores.v1.Core[]> {
		const cores = this.client
			.request<Archon.Cores.v1.Core[]>('/cores', { api: 'labrinth', version: 3, method: 'GET' })
			.then(async (list) => {
				const routes = await Promise.all(list.map((core) => this.route(core)))
				this.routes = new Map(list.map((core, index) => [core.id, routes[index]]))
				this.addresses = new Set(routes)
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
	 * Take a Core off the account. Its key stops working at once, and the Core signs itself out on
	 * its next call to the backend, so this works even while the Core is off. Its servers and files
	 * stay.
	 */
	public async revoke(core: Archon.Cores.v1.Core): Promise<void> {
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
		for (const [serverId, address] of this.serverAddresses) {
			if (address === this.base(core)) this.serverAddresses.delete(serverId)
		}
		this.addresses.delete(this.base(core))
		this.cores = null
	}

	/**
	 * Accept or decline an invite to manage a server on someone else's Core. The invite is what
	 * made that Core reachable, so the list is fetched again to find it.
	 */
	public async respondToInvite(
		coreId: string,
		serverId: string,
		action: 'accept' | 'decline',
	): Promise<void> {
		const core = (await this.list()).find((candidate) => candidate.id === coreId)
		if (!core) throw new ModrinthApiError('This invite is no longer available', { statusCode: 404 })
		await this.client.request(`/servers/${serverId}/invites/${action}`, {
			api: `${this.base(core)}/hosting`,
			version: 1,
			method: 'POST',
		})
		if (action === 'accept') this.serverAddresses.set(serverId, this.base(core))
		// Declining the only invite takes the Core out of the list.
		else this.cores = null
	}

	/** Whether `url` is on a listed Core, which is allowed to receive the account token. */
	public isCoreUrl(url: string): boolean {
		for (const address of this.addresses) {
			if (url.startsWith(address)) return true
		}
		return false
	}

	/** The hosting API base URL for a request: the Core that has the server the request names. */
	public async hostingUrl(path: string, params?: RequestOptions['params']): Promise<string> {
		const cores = await this.known()
		if (!cores.length) throw new ModrinthApiError('No Core is signed in', { statusCode: 404 })
		const serverId = cores.length > 1 && `${path} ${JSON.stringify(params ?? {})}`.match(SERVER_ID)
		if (serverId) {
			if (!this.serverAddresses.has(serverId[0])) await this.servers()
			const address = this.serverAddresses.get(serverId[0])
			if (address) return `${address}/hosting`
		}
		return `${this.base(cores[0])}/hosting`
	}

	/**
	 * The servers of every listed Core as one list. A Core that cannot be reached is left out and
	 * named in `offline_cores`; when none can be reached this throws.
	 */
	public async servers(
		options?: Archon.Servers.v0.GetServersOptions,
	): Promise<Archon.Servers.v0.ServerGetResponse> {
		const cores = await this.known()
		const results = await Promise.allSettled(
			cores.map((core) =>
				this.client.request<Archon.Servers.v0.ServerGetResponse>('/servers', {
					api: `${this.base(core)}/hosting`,
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
				this.serverAddresses.set(server.server_id, this.base(cores[index]))
				merged.servers.push(server)
			}
			Object.assign(merged.users, result.value.users)
		})
		const failure = results.find((result) => result.status === 'rejected')
		// A Core that did not answer may have moved or changed its certificate: ask the backend again.
		if (failure) this.cores = null
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
			api: `${this.base(core)}/hosting`,
			version: 'local',
			method: 'POST',
			body: { name, instance },
		})
		this.serverAddresses.set(server.id, this.base(core))
		return server
	}

	/**
	 * Whether the signed-in account may manage the servers of this Core: it is in the list of Cores
	 * the account may reach. Today that list holds only the account's own Cores.
	 */
	public async canReachCore(coreId: string): Promise<boolean> {
		return (await this.known()).some((core) => core.id === coreId)
	}

	private known(): Promise<Archon.Cores.v1.Core[]> {
		return this.cores ?? this.list()
	}

	/** The base URL requests to a listed Core go to. */
	private base(core: Archon.Cores.v1.Core): string {
		return this.routes.get(core.id) ?? core.address
	}

	/**
	 * Work out the base URL for a Core. Its local address is used when it answers, which is the
	 * case at home; otherwise its address on the internet. An HTTPS address goes through
	 * `connectCore`, which trusts only that Core's certificate.
	 */
	private async route(core: CoreLocation): Promise<string> {
		const connect = this.client.connectCore
		const open = (address: string) =>
			connect && core.cert_fingerprint && address.startsWith('https:')
				? connect(address, core.cert_fingerprint)
				: Promise.resolve(address)
		const local = await open(core.address)
		if (!core.public_address) return local
		const answered = await Promise.race([
			this.client
				.request('', {
					api: local,
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
