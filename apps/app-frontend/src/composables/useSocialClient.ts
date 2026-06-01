/**
 * Singleton ConvexApiClient (typed as AmberiteSocialClient) for the desktop app.
 *
 * Shares the same PlatformAdapter instance as the Core client so the dev
 * "act as" override and any future auth state stay consistent across both.
 */
import type { AmberiteSocialClient } from '@amberite/amberite-api'
import { ConvexApiClient } from '@amberite/amberite-api'

import { useCoreClient } from '@/composables/useCoreClient'

let _client: ConvexApiClient | null = null

export function useSocialClient(): AmberiteSocialClient {
	if (!_client) {
		_client = new ConvexApiClient(useCoreClient().adapter)
	}
	return _client
}

/** Raw client escape hatch for dev tooling that needs rawQuery/rawMutation. */
export function useSocialClientRaw(): ConvexApiClient {
	useSocialClient()
	return _client as ConvexApiClient
}
