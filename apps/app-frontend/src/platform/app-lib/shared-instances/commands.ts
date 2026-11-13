import {
	type CachedSharedInstanceSummary,
	instanceSharing,
	localSharing,
} from '@modrinth/api-client'

import { apiClient } from '@/platform/modrinth-client'

export type { CachedSharedInstanceSummary } from '@modrinth/api-client'

export function unpublishRemoteSharedInstance(sharedInstanceId: string): Promise<void> {
	return apiClient.local.call(instanceSharing.UnpublishRemoteSharedInstance, {
		params: { id: sharedInstanceId },
	})
}

export function leaveRemoteSharedInstance(sharedInstanceId: string): Promise<void> {
	return apiClient.local.call(instanceSharing.LeaveRemoteSharedInstance, {
		params: { id: sharedInstanceId },
	})
}

export function dismissSharedInstanceRestoredNotice(instanceId: string): Promise<void> {
	return apiClient.local.call(instanceSharing.DismissSharedInstanceRestoredNotice, {
		params: { id: instanceId },
	})
}

export function getCachedSharedInstanceLibrary(): Promise<CachedSharedInstanceSummary[]> {
	return apiClient.local.call(localSharing.getCachedSharedLibrary)
}

export function cacheSharedInstanceLibrary(
	instances: CachedSharedInstanceSummary[],
): Promise<void> {
	return apiClient.local.call(localSharing.setCachedSharedLibrary, { body: instances })
}
