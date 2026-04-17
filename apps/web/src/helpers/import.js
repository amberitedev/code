/**
 * Mocked import helper - returns empty/fake data
 * TODO: connect to backend API - replace with real API calls
 */

import { create } from './profile'

export async function get_importable_instances(launcherType, basePath) {
	return []
}

export async function import_instance(launcherType, basePath, instanceFolder) {
	const profilePath = await create(instanceFolder, '1.19.4', 'vanilla', 'latest', null, true)
	return profilePath
}

export async function is_valid_importable_instance(instanceFolder, launcherType) {
	return false
}

export async function get_default_launcher_path(launcherType) {
	return null
}
