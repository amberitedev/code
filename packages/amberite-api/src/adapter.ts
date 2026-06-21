/**
 * PlatformAdapter abstracts platform-owned capabilities such as fetch, local
 * Core discovery, auth token storage, and optional local durable queues.
 */
export interface PlatformAdapter {
	/** HTTP fetch function. On desktop this is tauriFetch; on web it is native fetch. */
	fetchFn: typeof fetch

	/** Convex deployment URL, e.g. https://...convex.cloud. */
	convexUrl: string

	/** Return the direct Core HTTP URL, e.g. "http://localhost:16662". */
	getCoreUrl(): Promise<string | null>

	/** Return the identity the current Core URL is linked to, when the platform has one. */
	getConnectedCoreId?(): Promise<string | null>

	/** Return the current user's auth JWT, if authenticated. */
	getCurrentJwt(): Promise<string | null>

	/** Persist a refreshed auth JWT when the auth provider returns one. */
	setCurrentJwt?(jwt: string | null): Promise<void>

	/** Return the Convex Auth refresh token paired with the current session JWT. */
	getCurrentRefreshToken?(): Promise<string | null>

	/** Persist the Convex Auth refresh token paired with the current session JWT. */
	setCurrentRefreshToken?(refreshToken: string | null): Promise<void>

	/** Return the one-time local setup secret for an app-launched Core, if present. */
	getLocalSetupSecret?(): Promise<string | null>

	/** Optional durable queue used by Mode 1a direct queued messages. */
	queueStore?: PersistentQueueStore

	/** Open an external auth window/redirect. Desktop opens system browser; web redirects page. */
	openExternalAuth(url: string): void | Promise<void>
}

export interface PersistentQueueStore {
	list(queueName: string): Promise<QueuedMessage[]>
	push(queueName: string, message: QueuedMessage): Promise<void>
	remove(queueName: string, id: string): Promise<void>
}

export interface QueuedMessage {
	id: string
	createdAt: number
	payload: unknown
}
