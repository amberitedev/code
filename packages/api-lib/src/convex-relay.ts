import type { PlatformAdapter } from './adapter'
import { NetworkError } from './errors'

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
	const res = await adapter.fetchFn(`${adapter.convexUrl}/api/${kind}`, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {}),
		},
		body: JSON.stringify({ path, args, format: 'json' }),
	})
	const body = await res.json().catch(() => null)
	if (!res.ok || body?.status !== 'success') {
		throw new NetworkError(body?.errorMessage ?? `Convex ${kind} failed: ${res.status}`)
	}
	return body.value as T
}
