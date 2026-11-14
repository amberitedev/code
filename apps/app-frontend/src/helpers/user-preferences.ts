import type { UserPreferencesContext } from '@modrinth/ui'

import { apiClient } from '@/platform/modrinth-client'

type UserPreferences = NonNullable<UserPreferencesContext['preferences']['value']>
type PartialUserPreferences = Parameters<UserPreferencesContext['updatePreferences']>[0]

export async function get_user_preferences(userId: string): Promise<UserPreferences> {
	return await apiClient.labrinth.users_v3.getPreferences(userId)
}

export async function patch_user_preferences(
	userId: string,
	preferences: PartialUserPreferences,
): Promise<UserPreferences> {
	return await apiClient.labrinth.users_v3.patchPreferences(userId, preferences)
}
