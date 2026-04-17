/**
 * Mocked pack helper - returns no-op
 * TODO: connect to backend API - replace with real pack installation
 */

import type { InstanceLoader } from './types'

export async function create_profile_and_install(
	_projectId: string,
	_versionId: string,
	_packTitle: string,
	_iconUrl?: string,
	_createInstanceCallback?: (profile: string) => void,
): Promise<void> {
	// no-op
}

export async function install_to_existing_profile(
	_projectId: string,
	_versionId: string,
	_title: string,
	_profilePath: string,
): Promise<void> {
	// no-op
}

export async function create_profile_and_install_from_file(_path: string): Promise<void> {
	// no-op
}
