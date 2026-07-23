/**
 * Singleton CoreApiClient for the desktop app.
 *
 * Call useCoreClient() anywhere to get the shared client instance.
 * The instance is created lazily on first access using the DesktopAdapter.
 */
import { CoreApiClient } from '@amberite/amberite-api'

import { createDesktopAdapter } from '@/adapters/desktop'

let _client: CoreApiClient | null = null
let _monitorStarted = false

export function useCoreClient(): CoreApiClient {
	if (!_client) {
		_client = new CoreApiClient(createDesktopAdapter())
	}
	return _client
}

export function startCoreMonitor(): void {
	const client = useCoreClient()
	if (!_monitorStarted) {
		_monitorStarted = true
		client.monitor
			?.start()
			.catch((e) => console.warn('[CoreApiClient] connection monitor failed:', e))
	}
}
