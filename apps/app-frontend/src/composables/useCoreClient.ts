/**
 * Singleton CoreApiClient for the desktop app.
 *
 * Call useCoreClient() anywhere to get the shared client instance.
 * The instance is created lazily on first access using the DesktopAdapter.
 */
import type { PlatformAdapter } from '@amberite/amberite-api'
import { CoreApiClient } from '@modrinth/api-client'

import { createDesktopCoreAdapter } from '@/adapters/desktop'

let _client: CoreApiClient | null = null
let _platformAdapter: ReturnType<typeof createDesktopCoreAdapter> | null = null
let _monitorStarted = false

export function useCoreClient(): CoreApiClient {
	if (!_client) {
		_platformAdapter = createDesktopCoreAdapter()
		_client = new CoreApiClient(_platformAdapter)
	}
	return _client
}

export function usePlatformAdapter(): PlatformAdapter {
	useCoreClient()
	return _platformAdapter!
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
