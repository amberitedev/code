import { onboarding } from '@modrinth/api-client'

import { apiClient } from '@/platform/modrinth-client'

export function skipCreateInstanceOnboarding(): Promise<void> {
	return apiClient.local.call(onboarding.skipCreateInstanceOnboarding)
}
