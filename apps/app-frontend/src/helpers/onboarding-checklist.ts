import { onboarding, type OnboardingChecklist } from '@modrinth/api-client'

import { apiClient } from '@/platform/modrinth-client'

export type { OnboardingChecklist }

export async function getOnboardingChecklist(): Promise<OnboardingChecklist> {
	return await apiClient.local.call(onboarding.getOnboardingChecklist)
}
