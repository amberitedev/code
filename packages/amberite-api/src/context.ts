/**
 * CoreCallContext is passed to every low-level API function in api.ts.
 * It carries the resolved base URL, auth token, and the platform fetch function.
 */
export interface CoreCallContext {
	baseUrl: string
	token: string | null
	fetchFn: typeof fetch
	timeoutMs?: number
	signal?: AbortSignal
}
