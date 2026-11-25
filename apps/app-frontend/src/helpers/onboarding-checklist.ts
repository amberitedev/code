import { onboarding, type OnboardingChecklist } from '@amberite/contracts/app'

import { apiClient } from '@/platform/modrinth-client'

export type { OnboardingChecklist }

export async function getOnboardingChecklist(): Promise<OnboardingChecklist> {
	return await apiClient.local.call(onboarding.getOnboardingChecklist)
}
