import { type Endpoint, PROTOCOL_VERSION } from '@amberite/contracts/protocol'
import { core as coreRoutes, type Health, TICKET_HEADER } from '@amberite/contracts/core'

import { ContractClient, type ContractTransport, fetchWithProgress } from '../contracts/client'
import { ApiCallError } from '../contracts/errors'
import type { AbstractModrinthClient } from '../core/abstract-client'

/** How long a Core call waits for its answer: a Core answers its own routes within 30 seconds. */
const CORE_TIMEOUT_MS = 35_000

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
		// A Core answers an untimed route only once its work is done, which can take any time.
		timeout: (endpoint) => (endpoint.untimed ? undefined : CORE_TIMEOUT_MS),
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
