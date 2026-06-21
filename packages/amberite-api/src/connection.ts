import type { PlatformAdapter } from './adapter'
import type { CoreConnectionHandshakeResponse } from './types'

export const CONNECTION_PROTOCOL = 1

export type ConnectionState = 'unknown' | 'connecting' | 'connected' | 'disconnected'
export type ConnectionFailureReason =
	| 'core-url-missing'
	| 'timeout'
	| 'network'
	| 'nonce-mismatch'
	| 'protocol-mismatch'
	| 'wrong-core'
	| 'rejected'

export interface ConnectionStatus {
	state: ConnectionState
	coreUrl: string | null
	coreId: string | null
	protocol: number | null
	version: string | null
	checkedAt: number
	reason?: ConnectionFailureReason
	error?: string
}

const HANDSHAKE_TIMEOUT_MS = 5_000

export async function verifyCoreConnection(
	adapter: PlatformAdapter,
	options: { coreUrl?: string | null; knownCoreId?: string | null; timeoutMs?: number } = {},
): Promise<ConnectionStatus> {
	const coreUrl = options.coreUrl ?? (await adapter.getCoreUrl())
	const checkedAt = Date.now()
	if (!coreUrl) {
		return emptyStatus('disconnected', checkedAt, 'core-url-missing')
	}

	const nonce = crypto.randomUUID()
	const controller = new AbortController()
	const timeout = setTimeout(() => controller.abort(), options.timeoutMs ?? HANDSHAKE_TIMEOUT_MS)

	try {
		const response = await adapter.fetchFn(`${coreUrl}/connection/handshake`, {
			method: 'POST',
			signal: controller.signal,
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({
				nonce,
				protocol: CONNECTION_PROTOCOL,
				known_core_id: options.knownCoreId ?? null,
			}),
		})

		if (!response.ok) {
			return emptyStatus('disconnected', checkedAt, 'network', coreUrl, `HTTP ${response.status}`)
		}

		const body = (await response.json()) as CoreConnectionHandshakeResponse
		if (body.nonce !== nonce) {
			return emptyStatus('disconnected', checkedAt, 'nonce-mismatch', coreUrl)
		}

		if (!body.ok) {
			return {
				state: 'disconnected',
				coreUrl,
				coreId: body.core_id,
				protocol: body.protocol,
				version: body.version,
				checkedAt,
				reason: body.reason ?? 'rejected',
			}
		}

		return {
			state: 'connected',
			coreUrl,
			coreId: body.core_id,
			protocol: body.protocol,
			version: body.version,
			checkedAt,
		}
	} catch (error) {
		return emptyStatus(
			'disconnected',
			checkedAt,
			error instanceof Error && error.name === 'AbortError' ? 'timeout' : 'network',
			coreUrl,
			error instanceof Error ? error.message : String(error),
		)
	} finally {
		clearTimeout(timeout)
	}
}

function emptyStatus(
	state: ConnectionState,
	checkedAt: number,
	reason: ConnectionFailureReason,
	coreUrl: string | null = null,
	error?: string,
): ConnectionStatus {
	return {
		state,
		coreUrl,
		coreId: null,
		protocol: null,
		version: null,
		checkedAt,
		reason,
		...(error ? { error } : {}),
	}
}
