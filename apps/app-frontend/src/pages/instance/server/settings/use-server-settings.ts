import { injectNotificationManager } from '@modrinth/ui'

import { useCoreClient } from '@/composables/useCoreClient'

import { injectCoreServerContext } from '../core-server-instance'

/**
 * Shared wiring for the forked server-settings tab pages: the Core client, the
 * current instance id, a server refresh, version/repair actions, and notifications.
 */
export function useServerSettings() {
	const core = useCoreClient()
	const context = injectCoreServerContext()
	const { addNotification, handleError } = injectNotificationManager()

	return {
		core,
		instanceId: context.instanceId,
		server: context.server,
		refreshServer: context.refreshServer,
		changeVersion: context.changeVersion,
		repairServer: context.repairServer,
		addNotification,
		handleError,
	}
}
