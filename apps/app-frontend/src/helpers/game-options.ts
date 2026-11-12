import {
	gameOptions,
	type GameOptionsSourceCandidate,
	type GameSettingLocaleLabel,
	type GameSettingsEditorState,
	type SaveGameSettingsResult,
	type UpdateGameSettingsRequest,
} from '@modrinth/api-client'
import { queryOptions } from '@tanstack/vue-query'

import { apiClient } from '@/platform/modrinth-client'

export type {
	EditableGameSetting,
	GameOptionCanonicalValue,
	GameOptionCompatibilityBucket,
	GameOptionCompatibilityReason,
	GameOptionCompatibilityStatus,
	GameOptionCompatibilitySummary,
	GameOptionEditor,
	GameOptionEditorChoice,
	GameOptionMappingKind,
	GameOptionsSourceCandidate,
	GameOptionsSourceDisabledReason,
	GameOptionValidationError,
	GameOptionValueState,
	GameSettingCategory,
	GameSettingChange,
	GameSettingLocaleLabel,
	GameSettingSource,
	GameSettingsEditorState,
	SaveGameSettingsResult,
	UpdateGameSettingsRequest,
} from '@modrinth/api-client'

export async function get_game_setting_locale_labels(
	instanceId: string | undefined,
	locale: string,
	optionIds: string[],
	refreshSources = false,
): Promise<{ settings: Record<string, GameSettingLocaleLabel> }> {
	return await apiClient.local.call(gameOptions.getGameSettingLabels, {
		body: {
			instance_id: instanceId,
			locale,
			option_ids: optionIds,
			refresh_sources: refreshSources,
		},
	})
}

export async function list_game_options_sync_sources(): Promise<GameOptionsSourceCandidate[]> {
	return await apiClient.local.call(gameOptions.listGameOptionsSources)
}

export async function get_synced_game_options_config(): Promise<GameSettingsEditorState> {
	return await apiClient.local.call(gameOptions.getSyncedGameOptions)
}

export const gameSettingsKeys = {
	synced: ['game-settings', 'synced'] as const,
	local: (instanceId: string) => ['game-settings', 'local', instanceId] as const,
}

export function gameSettingsQueryOptions(instanceId?: string) {
	return queryOptions({
		queryKey: instanceId ? gameSettingsKeys.local(instanceId) : gameSettingsKeys.synced,
		networkMode: 'always',
		queryFn: () =>
			instanceId ? get_local_game_options_config(instanceId) : get_synced_game_options_config(),
		staleTime: 30_000,
		retry: false,
	})
}

export async function preview_synced_game_option_changes(
	request: UpdateGameSettingsRequest,
): Promise<GameSettingsEditorState> {
	return await apiClient.local.call(gameOptions.previewSyncedGameOptions, { body: request })
}

export async function save_synced_game_option_changes(
	request: UpdateGameSettingsRequest,
): Promise<SaveGameSettingsResult> {
	return await apiClient.local.call(gameOptions.saveSyncedGameOptions, { body: request })
}

export async function get_local_game_options_config(
	instanceId: string,
): Promise<GameSettingsEditorState> {
	return await apiClient.local.call(gameOptions.getInstanceGameOptions, {
		params: { id: instanceId },
	})
}

export async function preview_local_game_option_changes(
	instanceId: string,
	request: UpdateGameSettingsRequest,
): Promise<GameSettingsEditorState> {
	return await apiClient.local.call(gameOptions.previewInstanceGameOptions, {
		params: { id: instanceId },
		body: request,
	})
}

export async function save_local_game_option_changes(
	instanceId: string,
	request: UpdateGameSettingsRequest,
): Promise<SaveGameSettingsResult> {
	return await apiClient.local.call(gameOptions.saveInstanceGameOptions, {
		params: { id: instanceId },
		body: request,
	})
}
