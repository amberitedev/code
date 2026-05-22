import { CoreInstanceStateManager } from '@amberite/amberite-api'
import { provideCoreClient, provideCoreInstanceState } from '@modrinth/ui'

import { getDesktopAdapter } from '@/adapters/desktop'

export function setupCoreInstanceState() {
	const manager = new CoreInstanceStateManager(getDesktopAdapter())
	provideCoreClient(manager.client)
	provideCoreInstanceState(manager)
	manager.start().catch((err) => {
		console.warn('[core-instance-state] failed to start', err)
	})
	return manager
}
