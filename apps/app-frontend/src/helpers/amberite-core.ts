import type { CoreMember, CoreMetadata, CoreSyncProfile } from '@amberite/amberite-api'
import { CoreApiClient } from '@amberite/amberite-api'
import { invoke } from '@tauri-apps/api/core'

import { getDesktopAdapter } from '@/adapters/desktop'

export type { CoreMember, CoreMetadata }
export type { CoreSyncProfile as SyncProfile }

export interface CoreAppSettings {
	core_url: string | null
	display_name: string | null
	auto_launch_core: boolean
}

export interface PermissionPreset {
	id: string
	name: string
	description: string
	role: 'owner' | 'admin' | 'member'
	permissions: { allow?: string[]; deny?: string[] } | Record<string, unknown>
}

export const coreClient = () => new CoreApiClient(getDesktopAdapter())

export async function getCoreAppSettings(): Promise<CoreAppSettings> {
	const settings = await invoke<Partial<CoreAppSettings>>('plugin:amberite|core_get_settings')
	return {
		core_url: settings.core_url ?? null,
		display_name: settings.display_name ?? null,
		auto_launch_core: settings.auto_launch_core ?? false,
	}
}

export async function setCoreAppSettings(settings: CoreAppSettings): Promise<void> {
	await invoke('plugin:amberite|core_set_settings', {
		coreUrl: settings.core_url,
		displayName: settings.display_name,
		autoLaunchCore: settings.auto_launch_core,
	})
}

export async function listPermissionPresets(): Promise<PermissionPreset[]> {
	return await invoke('plugin:amberite|core_list_permission_presets')
}

export async function isCoreInstalled(): Promise<boolean> {
	return await invoke('plugin:amberite|core_is_installed')
}

export async function startCore(): Promise<string> {
	return await invoke('plugin:amberite|core_start')
}

export async function installCore(downloadUrl: string): Promise<void> {
	await invoke('plugin:amberite|core_install', { downloadUrl })
}

export const getCoreMetadata = () => coreClient().getCoreMetadata()
export const updateCoreMetadata = (body: Partial<CoreMetadata>) =>
	coreClient().updateCoreMetadata(body)
export const listCoreMembers = () => coreClient().listCoreMembers()
export const upsertCoreMember = (body: Partial<CoreMember> & { user_id: string }) =>
	coreClient().upsertCoreMember(body)
export const removeCoreMember = (userId: string) => coreClient().removeCoreMember(userId)
export const listSyncProfiles = () => coreClient().listSyncProfiles()
export const registerSyncProfile = (body: Partial<CoreSyncProfile> & { name: string }) =>
	coreClient().registerSyncProfile(body)
export const removeSyncProfile = (profileId: string) => coreClient().removeSyncProfile(profileId)
