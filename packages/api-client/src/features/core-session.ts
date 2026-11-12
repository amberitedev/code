import {
	core as coreRoutes,
	type Endpoint,
	type Health,
	PROTOCOL_VERSION,
	TICKET_HEADER,
} from '@amberite/contracts'

import { ContractClient, type ContractTransport } from '../contracts/client'
import type { AbstractModrinthClient } from '../core/abstract-client'
import { AbstractFeature } from '../core/abstract-feature'
import { ModrinthApiError } from '../core/errors'
import type { RequestContext } from '../types/request'

/**
 * Requests to a listed Core. The client adds this itself when `ClientConfig.coreSessions` is set,
 * innermost, so it acts on each attempt as it is sent:
 *
 * - A `dangerous` request gets a fresh ticket in `TICKET_HEADER`.
 * - A request Core answers with 401 `invalid_credentials` (no session, or it ended) signs in with a
 *   new ticket once and is sent again. Any other 401, such as a refused ticket, is the answer.
 */
export class CoreSessionFeature extends AbstractFeature {
	constructor(private readonly client: AbstractModrinthClient) {
		super({ name: 'core-session' })
	}

	shouldApply(context: RequestContext): boolean {
		return (
			super.shouldApply(context) &&
			this.client.archon.cores_v1.coreForUrl(context.url) !== undefined
		)
	}

	async execute<T>(next: () => Promise<T>, context: RequestContext): Promise<T> {
		const cores = this.client.archon.cores_v1
		const core = cores.coreForUrl(context.url)
		if (!core) return next()
		const send = async () => {
			if (context.options.dangerous)
				context.options.headers = {
					...context.options.headers,
					[TICKET_HEADER]: await cores.ticket(core.id),
				}
			return next()
		}
		try {
			return await send()
		} catch (error) {
			if (!signedOut(error)) throw error
			await cores.signIn(core)
			return send()
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

/**
 * How `client.core(id)` reaches a listed Core by default: at the base `archon.cores_v1` routes it
 * to, where the platform adds this device's session. A dangerous call carries a fresh ticket, and
 * a call the Core answers with 401 `invalid_credentials` signs in once and is sent again; the
 * Core checks the session before the ticket, so the ticket is still unused.
 */
export function coreSessionTransport(
	client: AbstractModrinthClient,
	coreId: string,
	fetch: typeof globalThis.fetch,
): ContractTransport {
	const cores = client.archon.cores_v1
	return {
		base: () => cores.url(coreId),
		headers: async (endpoint) => {
			const headers: Record<string, string> = {}
			if (endpoint.dangerous) headers[TICKET_HEADER] = await cores.ticket(coreId)
			return headers
		},
		fetch: async (input, init) => {
			const response = await fetch(input, init)
			if (!(await answeredSignedOut(response))) return response
			await cores.signIn(await cores.core(coreId))
			return fetch(input, init)
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
 * every call but the `STABLE` ones throws `ProtocolMismatch`. `owner` says whether the signed-in
 * account owns the Core.
 */
export function protocolChecked(
	transport: ContractTransport,
	owner: () => Promise<boolean>,
): ContractTransport {
	const core = new ContractClient(transport)
	let compatible: Promise<void> | undefined
	const check = async () => {
		const mismatch = protocolMismatch(await core.call(coreRoutes.CheckHealth), await owner())
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

/** A Core speaks another protocol version than this App. `message` says who can fix it. */
export class ProtocolMismatch extends Error {
	constructor(
		/** The side that is too old. */
		readonly side: 'core' | 'app',
		/** The signed-in account owns the Core, so it can update it. */
		readonly owner: boolean,
	) {
		super(
			side === 'app'
				? 'This Core is newer than this App. Update Amberite.'
				: owner
					? 'This Core is out of date. Update this Core.'
					: 'This Core is out of date. Ask the owner to update it.',
		)
		this.name = 'ProtocolMismatch'
	}
}

/** How a Core's `health` differs from `ours`, if it does. A Core that reports none speaks 1. */
export function protocolMismatch(
	health: Pick<Health, 'protocol'>,
	owner: boolean,
	ours: number = PROTOCOL_VERSION,
): ProtocolMismatch | null {
	const theirs = health.protocol ?? 1
	if (theirs === ours) return null
	return new ProtocolMismatch(theirs < ours ? 'core' : 'app', owner)
}
