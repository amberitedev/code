import {
	core as coreRoutes,
	type Endpoint,
	type Health,
	installs,
	instanceContent,
	instanceFiles,
	logs,
	PROTOCOL_VERSION,
	TICKET_HEADER,
} from '@amberite/contracts'

import { ContractClient, type ContractTransport, fetchWithProgress } from '../contracts/client'
import { ApiCallError } from '../contracts/errors'
import type { AbstractModrinthClient } from '../core/abstract-client'
import { AbstractFeature } from '../core/abstract-feature'
import { ModrinthApiError } from '../core/errors'
import type { RequestContext } from '../types/request'

/**
 * Requests to a listed Core. The client adds this itself when `ClientConfig.coreSessions` is set.
 * A request Core answers with 401 `invalid_credentials` (no session, or it ended) signs in with a
 * new ticket once and is sent again. Any other 401, such as a refused ticket, is the answer.
 */
export class CoreSessionFeature extends AbstractFeature {
	constructor(private readonly client: AbstractModrinthClient) {
		super({ name: 'core-session' })
	}

	shouldApply(context: RequestContext): boolean {
		return super.shouldApply(context) && this.client.cores.coreForUrl(context.url) !== undefined
	}

	async execute<T>(next: () => Promise<T>, context: RequestContext): Promise<T> {
		const cores = this.client.cores
		const core = cores.coreForUrl(context.url)
		if (!core) return next()
		try {
			return await next()
		} catch (error) {
			if (!signedOut(error)) throw error
			await cores.signIn(core)
			return next()
		}
	}
}

function signedOut(error: unknown): boolean {
	if (!(error instanceof ModrinthApiError) || error.statusCode !== 401) return false
	const data = error.responseData
	return (
		typeof data === 'object' &&
		data !== null &&
		'error' in data &&
		data.error === 'invalid_credentials'
	)
}

/** How long a Core call waits for its answer: a Core answers its own routes within 30 seconds. */
const CORE_TIMEOUT_MS = 35_000

/**
 * Routes a Core answers only once their work is done, which can take any time: the engine's
 * long routes (`long_routes` in `apps/core/src/api/engine_mount.rs`), a push, following an
 * instance, and installing an update. Long ops of a Core's own answer 202 at once instead.
 */
const UNTIMED = new Set<Endpoint>([
	instanceContent.GetLinkedModpack,
	instanceContent.AddProjectFromVersion,
	instanceContent.InstallProjectWithDependencies,
	instanceContent.UpdateProject,
	instanceContent.SwitchProjectVersion,
	instanceContent.RepairContent,
	instanceContent.RepairModpack,
	instanceContent.InstallModpack,
	instanceContent.UpdateModpackVersion,
	installs.UpdateInstanceContent,
	installs.RestoreSharedInstanceVersion,
	instanceFiles.ReadInstanceFile,
	instanceFiles.ExtractInstanceZip,
	instanceFiles.ZipInstanceFiles,
	instanceFiles.FinalizeInstanceUpload,
	logs.ListInstanceLogs,
	logs.GetInstanceLogOutput,
	coreRoutes.ShareServer,
	coreRoutes.FollowInstance,
	coreRoutes.InstallUpdate,
])

/**
 * How `client.core(id)` reaches a listed Core by default: at the base `cores` routes it
 * to, where the platform adds this device's session. A dangerous call carries a fresh ticket, and
 * a call the Core answers with 401 `invalid_credentials` signs in once and is sent again; the
 * Core checks the session before the ticket, so the ticket is still unused.
 */
export function coreSessionTransport(
	client: AbstractModrinthClient,
	coreId: string,
	fetch: typeof globalThis.fetch,
): ContractTransport {
	const cores = client.cores
	return {
		base: () => cores.url(coreId),
		timeout: (endpoint) => (UNTIMED.has(endpoint) ? undefined : CORE_TIMEOUT_MS),
		headers: async (endpoint) => {
			const headers: Record<string, string> = {}
			if (endpoint.dangerous) headers[TICKET_HEADER] = await cores.ticket(coreId)
			return headers
		},
		fetch: async (input, init) => {
			// Only the page's own XMLHttpRequest reports an upload's progress.
			const send = () =>
				init.onUploadProgress ? fetchWithProgress(input, init) : fetch(input, init)
			const response = await send()
			if (!(await answeredSignedOut(response))) return response
			await cores.signIn(await cores.core(coreId))
			return send()
		},
	}
}

async function answeredSignedOut(response: Response): Promise<boolean> {
	if (response.status !== 401) return false
	const body: unknown = await response
		.clone()
		.json()
		.catch(() => undefined)
	return typeof body === 'object' && body !== null && 'error' in body
		? body.error === 'invalid_credentials'
		: false
}

/** Every protocol version keeps these, so an outdated Core can still be checked and updated. */
const STABLE = new Set<Endpoint>([coreRoutes.CheckHealth, coreRoutes.InstallUpdate])

/**
 * `transport`, checking the Core's protocol before its first call. Once it differs from ours,
 * every call but the `STABLE` ones throws `ProtocolMismatch`.
 */
export function protocolChecked(transport: ContractTransport): ContractTransport {
	const core = new ContractClient(transport)
	let compatible: Promise<void> | undefined
	const check = async () => {
		const mismatch = protocolMismatch(await core.call(coreRoutes.CheckHealth))
		if (mismatch) throw mismatch
	}
	return {
		...transport,
		headers: async (endpoint) => {
			if (!STABLE.has(endpoint)) {
				// A failed check runs again on the next call, after the Core may have been updated.
				compatible ??= check().catch((error: unknown) => {
					compatible = undefined
					throw error
				})
				await compatible
			}
			return transport.headers(endpoint)
		},
	}
}

/** A Core speaks another protocol version than this App. `description` says which to update. */
export class ProtocolMismatch extends ApiCallError {
	constructor(
		/** The side that is too old. */
		readonly side: 'core' | 'app',
	) {
		super(
			0,
			'unsupported',
			side === 'app'
				? 'This Core is newer than this App. Update Amberite.'
				: 'This Core is out of date. Update this Core.',
		)
		this.name = 'ProtocolMismatch'
	}
}

/** How a Core's `health` differs from `ours`, if it does. */
export function protocolMismatch(
	health: Pick<Health, 'protocol'>,
	ours: number = PROTOCOL_VERSION,
): ProtocolMismatch | null {
	const theirs = health.protocol
	if (theirs === ours) return null
	return new ProtocolMismatch(theirs < ours ? 'core' : 'app')
}
