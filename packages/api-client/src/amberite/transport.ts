import { authErrorFromResponse, AmberiteNetworkError } from './errors'
import type { AmberitePlatformAdapter } from './platform'
import { refreshPlatformAmberiteSession } from './session'

export interface AmberiteTransport {
	query<T = unknown>(path: string, args?: unknown): Promise<T>
	mutation<T = unknown>(path: string, args?: unknown): Promise<T>
	action<T = unknown>(path: string, args?: unknown, authenticate?: boolean): Promise<T>
}

type ConvexKind = 'query' | 'mutation' | 'action'
type ConvexResponse = { status?: unknown; value?: unknown; errorMessage?: unknown }

export class ConvexAmberiteTransport implements AmberiteTransport {
	constructor(private readonly adapter: AmberitePlatformAdapter) {}

	query<T = unknown>(path: string, args: unknown = {}): Promise<T> {
		return this.call('query', path, args, true)
	}

	mutation<T = unknown>(path: string, args: unknown = {}): Promise<T> {
		return this.call('mutation', path, args, true)
	}

	action<T = unknown>(path: string, args: unknown = {}, authenticate = true): Promise<T> {
		return this.call('action', path, args, authenticate)
	}

	private async call<T>(kind: ConvexKind, path: string, args: unknown, authenticate: boolean) {
		let response = await this.request(kind, path, args, authenticate)
		if (authenticate && unauthorized(response) && this.adapter.refreshAmberiteSession) {
			const refreshed = await refreshPlatformAmberiteSession(this.adapter)
			if (refreshed?.accessToken) {
				await this.adapter.setCurrentJwt?.(refreshed.accessToken)
				response = await this.request(kind, path, args, true)
			}
		}
		if (!response.http.ok || response.body?.status !== 'success') {
			const message =
				typeof response.body?.errorMessage === 'string'
					? response.body.errorMessage
					: `Convex ${kind} failed: ${response.http.status}`
			throw authErrorFromResponse(message, response.http.status)
		}
		return response.body.value as T
	}

	private async request(
		kind: ConvexKind,
		path: string,
		args: unknown,
		authenticate: boolean,
	): Promise<{ http: Response; body: ConvexResponse | null }> {
		if (!this.adapter.convexUrl) throw new AmberiteNetworkError('Convex URL is not configured')
		const token = authenticate ? await this.adapter.getCurrentJwt() : null
		const controller = new AbortController()
		const timeout = setTimeout(() => controller.abort(), 15_000)
		try {
			const http = await this.adapter.fetchFn(`${this.adapter.convexUrl}/api/${kind}`, {
				method: 'POST',
				signal: controller.signal,
				headers: {
					'Content-Type': 'application/json',
					...(token ? { Authorization: `Bearer ${token}` } : {}),
				},
				body: JSON.stringify({ path, args, format: 'json' }),
			})
			const body: unknown = await http.json().catch(() => null)
			return {
				http,
				body: body && typeof body === 'object' ? (body as ConvexResponse) : null,
			}
		} catch (error) {
			if (error instanceof Error && error.name === 'AbortError')
				throw new AmberiteNetworkError(`Convex ${kind} timed out`, 'timeout')
			throw new AmberiteNetworkError(
				error instanceof Error ? error.message : `Convex ${kind} request failed`,
				'offline',
			)
		} finally {
			clearTimeout(timeout)
		}
	}
}

function unauthorized(response: { http: Response; body: ConvexResponse | null }) {
	return (
		response.http.status === 401 ||
		response.http.status === 403 ||
		String(response.body?.errorMessage ?? '')
			.toLowerCase()
			.includes('not authenticated')
	)
}
