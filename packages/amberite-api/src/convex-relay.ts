import type { PlatformAdapter } from './adapter'
import { NetworkError } from './errors'

const CONVEX_REQUEST_TIMEOUT_MS = 15_000

/** Convex args are always a plain JSON object; only then can we add `__actAs`. */
function isPlainArgs(args: unknown): args is Record<string, unknown> {
	return typeof args === 'object' && args !== null && !Array.isArray(args)
}

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

async function convexCall<T>(
	adapter: PlatformAdapter,
	kind: 'query' | 'mutation',
	path: string,
	args: unknown,
): Promise<T> {
	const token = await adapter.getCurrentJwt()
	const devActingUserId = adapter.getDevActingUserId?.()
	const finalArgs =
		devActingUserId && isPlainArgs(args) ? { ...(args as object), __actAs: devActingUserId } : args
	const controller = new AbortController()
	const timeout = setTimeout(() => controller.abort(), CONVEX_REQUEST_TIMEOUT_MS)
	let res: Response
	try {
		res = await adapter.fetchFn(`${adapter.convexUrl}/api/${kind}`, {
			method: 'POST',
			signal: controller.signal,
			headers: {
				'Content-Type': 'application/json',
				...(token ? { Authorization: `Bearer ${token}` } : {}),
			},
			body: JSON.stringify({ path, args: finalArgs, format: 'json' }),
		})
	} catch (err) {
		if (err instanceof Error && err.name === 'AbortError') {
			throw new NetworkError(`Convex ${kind} timed out`)
		}
		throw err
	} finally {
		clearTimeout(timeout)
	}
	const body = await res.json().catch(() => null)
	if (!res.ok || body?.status !== 'success') {
		throw new NetworkError(body?.errorMessage ?? `Convex ${kind} failed: ${res.status}`)
	}
	return body.value as T
}
