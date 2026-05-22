import { getDesktopAdapter } from '@/adapters/desktop'

import { ensureAmberiteSession } from './amberite-auth'

export async function convexQuery<T = unknown>(path: string, args: unknown = {}): Promise<T> {
	return await convexCall<T>('query', path, args)
}

export async function convexMutation<T = unknown>(path: string, args: unknown = {}): Promise<T> {
	return await convexCall<T>('mutation', path, args)
}

async function convexCall<T>(kind: 'query' | 'mutation', path: string, args: unknown): Promise<T> {
	const adapter = getDesktopAdapter()
	const token = await ensureAmberiteSession()
	const response = await adapter.fetchFn(`${adapter.convexUrl}/api/${kind}`, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {}),
		},
		body: JSON.stringify({ path, args, format: 'json' }),
	})
	const body = await response.json().catch(() => null)
	if (!response.ok || body?.status !== 'success') {
		throw new Error(body?.errorMessage ?? `Convex ${kind} failed: ${response.status}`)
	}
	return body.value as T
}
