export interface CoreCallContext {
	baseUrl: string
	token: string | null
	fetchFn: typeof fetch
	timeoutMs?: number
	signal?: AbortSignal
}
