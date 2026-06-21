// TODO: migrate to content-install.ts DI

import {
	findPreferredVersion,
	installVersionDependencies as installVersionDependenciesWithAdapters,
	isVersionCompatible,
} from '@amberite/amberite-api'

import { get_project, get_version, get_version_many } from '@/helpers/cache.js'
import { add_project_from_version, check_installed } from '@/helpers/profile.js'
import {
	add_server_to_profile,
	get_profile_worlds,
	resolveManagedServerWorld,
} from '@/helpers/worlds.ts'

export { findPreferredVersion, isVersionCompatible }

export const installVersionDependencies = async (profile, version, reason, onDepInstalling) => {
	await installVersionDependenciesWithAdapters({
		profile,
		version,
		reason,
		getProject: (projectId) => get_project(projectId, 'bypass').catch(() => null),
		getVersion: (versionId) => get_version(versionId, 'bypass').catch(() => null),
		getVersions: (versionIds) => get_version_many(versionIds, 'bypass').catch(() => []),
		isProjectInstalled: check_installed,
		installVersion: add_project_from_version,
		onDependencyInstalling: onDepInstalling,
	})
}

export const getServerAddress = (javaServer) => {
	if (!javaServer) return null
	const { address } = javaServer
	return address
}

export const ensureManagedServerWorldExists = async (profilePath, serverName, serverAddress) => {
	if (!profilePath || !serverAddress) return
	try {
		const worlds = await get_profile_worlds(profilePath)
		const managedWorld = resolveManagedServerWorld(worlds, serverName, serverAddress)
		if (!managedWorld) {
			await add_server_to_profile(profilePath, serverName, serverAddress, 'prompt')
		}
	} catch (err) {
		console.error('Failed to ensure managed server world exists:', err)
	}
}
