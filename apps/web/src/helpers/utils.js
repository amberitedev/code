/**
 * Mocked utils helper - returns empty/fake data
 * TODO: connect to backend API - replace with real API calls
 */

import { get_full_path, get_mod_full_path } from './profile'

export async function isDev() {
	return false
}

export async function areUpdatesEnabled() {
	return false
}

export async function getUpdateSize(_updateRid) {
	return 0
}

export async function enqueueUpdateForInstallation(_updateRid) {
	// no-op
}

export async function removeEnqueuedUpdate() {
	// no-op
}

export async function getOS() {
	return 'web'
}

export async function isNetworkMetered() {
	return false
}

export async function openPath(_path) {
	// no-op
}

export async function highlightInFolder(_path) {
	// no-op
}

export async function showLauncherLogsFolder() {
	// no-op
}

export async function showProfileInFolder(_path) {
	// no-op
}

export async function highlightModInProfile(_profilePath, _projectPath) {
	// no-op
}

export async function restartApp() {
	// no-op - in web mode, just reload
	window.location.reload()
}

export const releaseColor = (releaseType) => {
	switch (releaseType) {
		case 'release':
			return 'green'
		case 'beta':
			return 'orange'
		case 'alpha':
			return 'red'
		default:
			return ''
	}
}

export async function copyToClipboard(text) {
	await navigator.clipboard.writeText(text)
}
