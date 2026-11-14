import { library } from '@modrinth/api-client'

import { apiClient } from '@/platform/modrinth-client'

export function getHiddenInstanceIds(): Promise<string[]> {
	return apiClient.local.call(library.listHiddenInstances)
}

export function setInstanceHidden(instanceId: string, hidden: boolean): Promise<void> {
	return apiClient.local.call(library.setInstanceHidden, {
		body: { instance_id: instanceId, hidden },
	})
}
