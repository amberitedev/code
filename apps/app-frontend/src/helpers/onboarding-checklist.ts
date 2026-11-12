import type { OnboardingChecklist } from '@modrinth/api-client'
import { invoke } from '@tauri-apps/api/core'

export type { OnboardingChecklist }

export async function getOnboardingChecklist(): Promise<OnboardingChecklist> {
	return await invoke('plugin:onboarding-checklist|get_onboarding_checklist')
}
