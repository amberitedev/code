import type { PlatformAdapter } from './adapter'
import { authErrorFromResponse, NetworkError } from './errors'

const CONVEX_REQUEST_TIMEOUT_MS = 15_000
const refreshes = new WeakMap<PlatformAdapter, Promise<{ accessToken: string } | null>>()

export async function convexQuery<T = unknown>(
	adapter: PlatformAdapter,
	path: string,
	args: unknown,
): Promise<T> {
	return await convexCall<T>(adapter, 'query', path, args)
}

export async function convexMutation<T = unknown>(
	adapter: PlatformAdapter,
	path: string,
	args: unknown,
): Promise<T> {
	return await convexCall<T>(adapter, 'mutation', path, args)
}

export async function convexAction<T = unknown>(
	adapter: PlatformAdapter,
	path: string,
	args: unknown,
	authenticate = true,
): Promise<T> {
	return await convexCall<T>(adapter, 'action', path, args, authenticate)
}

async function convexCall<T>(
	adapter: PlatformAdapter,
	kind: 'query' | 'mutation' | 'action',
	path: string,
	args: unknown,
	authenticate = true,
): Promise<T> {
	if (!adapter.convexUrl) throw new NetworkError('Convex URL is not configured')

	let response = await request(adapter, kind, path, args, authenticate)
	if (authenticate && isUnauthorized(response) && adapter.refreshAmberiteSession) {
		const refreshed = await refreshOnce(adapter)
		if (refreshed?.accessToken) {
			await adapter.setCurrentJwt?.(refreshed.accessToken)
			response = await request(adapter, kind, path, args, true)
		}
	}

	if (!response.res.ok || response.body?.status !== 'success') {
		const message = response.body?.errorMessage ?? `Convex ${kind} failed: ${response.res.status}`
		throw authErrorFromResponse(message, response.res.status)
	}
	return response.body.value as T
}

async function request(
	adapter: PlatformAdapter,
	kind: 'query' | 'mutation' | 'action',
	path: string,
	args: unknown,
	authenticate: boolean,
): Promise<{ res: Response; body: any }> {
	const token = authenticate ? await adapter.getCurrentJwt() : null
	const controller = new AbortController()
	const timeout = setTimeout(() => controller.abort(), CONVEX_REQUEST_TIMEOUT_MS)
	try {
		const res = await adapter.fetchFn(`${adapter.convexUrl}/api/${kind}`, {
			method: 'POST',
			signal: controller.signal,
			headers: {
				'Content-Type': 'application/json',
				...(token ? { Authorization: `Bearer ${token}` } : {}),
			},
			body: JSON.stringify({ path, args, format: 'json' }),
		})
		return { res, body: await res.json().catch(() => null) }
	} catch (error) {
		if (error instanceof Error && error.name === 'AbortError') {
			throw new NetworkError(`Convex ${kind} timed out`, 'timeout')
		}
		throw new NetworkError(
			error instanceof Error ? error.message : `Convex ${kind} request failed`,
			'offline',
		)
	} finally {
		clearTimeout(timeout)
	}
}

async function refreshOnce(adapter: PlatformAdapter): Promise<{ accessToken: string } | null> {
	const existing = refreshes.get(adapter)
	if (existing) return await existing
	const refresh = adapter.refreshAmberiteSession!().finally(() => refreshes.delete(adapter))
	refreshes.set(adapter, refresh)
	return await refresh
}

function isUnauthorized(response: { res: Response; body: any }): boolean {
	return (
		response.res.status === 401 ||
		response.res.status === 403 ||
		String(response.body?.errorMessage ?? '')
			.toLowerCase()
			.includes('not authenticated')
	)
}
