/**
 * Desktop Convex transports. Durable screen state is subscribed through the
 * singleton WebSocket client; HTTP remains available for auth and one-shot
 * action-oriented calls.
 */
import type { AmberiteSocialClient } from '@amberite/amberite-api'
import { ConvexApiClient } from '@amberite/amberite-api'
import { ConvexClient } from 'convex/browser'

import { useCoreClient } from '@/composables/useCoreClient'
import { config } from '@/config'

let socialClient: ConvexApiClient | null = null
let realtimeClient: ConvexClient | null = null

export function useSocialClient(): AmberiteSocialClient {
	if (!socialClient) socialClient = new ConvexApiClient(useCoreClient().adapter)
	return socialClient
}

/** Raw HTTP client for the Convex Auth bootstrap endpoints. */
export function useSocialClientRaw(): ConvexApiClient {
	useSocialClient()
	return socialClient as ConvexApiClient
}

export function useRealtimeConvexClient(): ConvexClient {
	if (!realtimeClient) {
		if (!config.convexUrl) throw new Error('VITE_CONVEX_URL must be configured for realtime social state.')
		realtimeClient = new ConvexClient(config.convexUrl, { unsavedChangesWarning: false })
		realtimeClient.setAuth(async () => await useCoreClient().adapter.getCurrentJwt())
	}
	return realtimeClient
}

export function refreshRealtimeConvexAuth(): void {
	const client = useRealtimeConvexClient()
	client.setAuth(async () => await useCoreClient().adapter.getCurrentJwt())
}
