import { useStorage } from '@vueuse/core'

/**
 * Whether the Getting started checklist was dismissed from the sidebar. Local to this install; the
 * checklist itself still completes in the background. Shared so the Features setting can bring the
 * checklist back.
 */
export const onboardingChecklistDismissed = useStorage('onboarding-checklist-dismissed', false)
