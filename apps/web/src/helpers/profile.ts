/**
 * Mocked profile helper - returns empty/fake data
 * Original: calls Tauri/Rust backend for profile management
 */

import type { Labrinth } from '@modrinth/api-client'
import type { ContentItem, ContentOwner } from '@modrinth/ui'

import type {
	CacheBehaviour,
	ContentFile,
	ContentFileProjectType,
	GameInstance,
	InstanceLoader,
} from './types'

export async function create(
	_name: string,
	_gameVersion: string,
	_modloader: InstanceLoader,
	_loaderVersion: string | null,
	_icon: string | null,
	_skipInstall: boolean,
	_linkedData?: { project_id: string; version_id: string; locked: boolean } | null,
): Promise<string> {
	return ''
}

export async function duplicate(_path: string): Promise<string> {
	return ''
}

export async function remove(_path: string): Promise<void> {
	// no-op
}

export async function get(_path: string): Promise<GameInstance | null> {
	// TODO: connect to backend API - return real instance data
	return {
		path: _path,
		install_stage: 'not_installed',
		name: 'New Instance',
		game_version: '',
		loader: 'vanilla',
		groups: [],
		created: new Date(),
		modified: new Date(),
		submitted_time_played: 0,
		recent_time_played: 0,
		hooks: {},
	}
}

export async function get_many(_paths: string[]): Promise<GameInstance[]> {
	return []
}

export async function get_projects(_path: string, _cacheBehaviour?: CacheBehaviour): Promise<Record<string, ContentFile>> {
	return {}
}

export async function get_installed_project_ids(_path: string): Promise<string[]> {
	return []
}

export async function get_content_items(_path: string, _cacheBehaviour?: CacheBehaviour): Promise<ContentItem[]> {
	return []
}

export interface LinkedModpackInfo {
	project: Labrinth.Projects.v2.Project
	version: Labrinth.Versions.v2.Version
	owner: ContentOwner | null
	has_update: boolean
	update_version_id: string | null
	update_version: Labrinth.Versions.v2.Version | null
}

export async function get_linked_modpack_info(
	_path: string,
	_cacheBehaviour?: CacheBehaviour,
): Promise<LinkedModpackInfo | null> {
	return null
}

export async function get_linked_modpack_content(
	_path: string,
	_cacheBehaviour?: CacheBehaviour,
): Promise<ContentItem[]> {
	return []
}

export async function get_dependencies_as_content_items(
	_dependencies: Labrinth.Versions.v3.Dependency[],
	_cacheBehaviour?: CacheBehaviour,
): Promise<ContentItem[]> {
	return []
}

export async function get_full_path(_path: string): Promise<string> {
	return ''
}

export async function get_mod_full_path(_path: string, _projectPath: string): Promise<string> {
	return ''
}

export async function get_optimal_jre_key(_path: string): Promise<string | null> {
	return null
}

export async function list(): Promise<GameInstance[]> {
	return []
}

export async function check_installed(_path: string, _projectId: string): Promise<boolean> {
	return false
}

export async function check_installed_batch(_projectId: string): Promise<Record<string, boolean>> {
	return {}
}

export async function install(_path: string, _force: boolean): Promise<void> {
	// no-op
}

export async function update_all(_path: string): Promise<Record<string, string>> {
	return {}
}

export async function update_project(_path: string, _projectPath: string): Promise<string> {
	return ''
}

export async function add_project_from_version(_path: string, _versionId: string): Promise<string> {
	return ''
}

export async function add_project_from_path(
	_path: string,
	_projectPath: string,
	_projectType?: ContentFileProjectType,
): Promise<string> {
	return ''
}

export async function toggle_disable_project(_path: string, _projectPath: string): Promise<string> {
	return ''
}

export async function remove_project(_path: string, _projectPath: string): Promise<void> {
	// no-op
}

export async function update_managed_modrinth_version(_path: string, _versionId: string): Promise<void> {
	// no-op
}

export async function update_repair_modrinth(_path: string): Promise<void> {
	// no-op
}

export async function export_profile_mrpack(
	_path: string,
	_exportLocation: string,
	_includedOverrides: string[],
	_versionId?: string,
	_description?: string,
	_name?: string,
): Promise<void> {
	// no-op
}

export async function get_pack_export_candidates(_profilePath: string): Promise<string[]> {
	return []
}

export async function run(_path: string, _serverAddress: string | null = null): Promise<unknown> {
	return null
}

export async function kill(_path: string): Promise<void> {
	// no-op
}

export async function edit(_path: string, _editProfile: Partial<GameInstance>): Promise<void> {
	// no-op
}

export async function edit_icon(_path: string, _iconPath: string | null): Promise<void> {
	// no-op
}

export async function finish_install(_instance: GameInstance): Promise<void> {
	// no-op
}
