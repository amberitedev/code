import { desktop, updates } from '@amberite/contracts/app'

import { get_full_path, get_mod_full_path } from '@/helpers/instance'
import { save } from '@/platform/dialog'
import { apiClient } from '@/platform/modrinth-client'
import { runOp } from '@/platform/ops'
import { getBoot } from '@/platform/os'

export async function isDev() {
	return getBoot().dev
}

export async function areUpdatesEnabled() {
	return getBoot().updates_enabled
}

export async function checkForUpdate() {
	return await apiClient.local.call(updates.checkForUpdate)
}

// The App keeps the update its last check found, so the rid is unused.
export async function getUpdateSize(_updateRid) {
	return await apiClient.local.call(updates.getUpdateSize)
}

export async function enqueueUpdateForInstallation(_updateRid) {
	const op = crypto.randomUUID()
	const { error } = await runOp(op, () =>
		apiClient.local.call(updates.downloadUpdate, { body: { op } }),
	)
	if (error) throw new Error(error.description)
}

export async function removeEnqueuedUpdate() {
	return await apiClient.local.call(updates.discardDownloadedUpdate)
}

export async function setRestartAfterPendingUpdate(should_restart) {
	return await apiClient.local.call(updates.setRestartAfterUpdate, {
		body: { restart: should_restart },
	})
}

// One of 'Windows', 'Linux', 'MacOS'
export async function getOS() {
	return { windows: 'Windows', macos: 'MacOS', linux: 'Linux' }[getBoot().platform]
}

export async function isNetworkMetered() {
	return await apiClient.local.call(desktop.checkNetworkMetered)
}

export async function openPath(path) {
	return await apiClient.local.call(desktop.openPath, { body: { path } })
}

export async function highlightInFolder(path) {
	return await apiClient.local.call(desktop.revealPath, { body: { path } })
}

export async function showLauncherLogsFolder() {
	return await apiClient.local.call(desktop.openFolder, { body: { folder: 'launcher_logs' } })
}

export async function createInstanceShortcut(instanceName, instanceId, options = {}) {
	const outputPath = await save({
		defaultPath: `Modrinth - ${instanceName}`,
	})

	if (!outputPath) return null

	return await apiClient.local.call(desktop.createShortcut, {
		body: {
			instance_name: instanceName,
			instance_id: instanceId,
			output_path: outputPath,
			server: options.server ?? null,
			singleplayer_world: options.singleplayerWorld ?? null,
		},
	})
}

export async function showAppDbBackupsFolder() {
	return await apiClient.local.call(desktop.openFolder, { body: { folder: 'app_db_backups' } })
}

// Opens an instance's folder in the OS file explorer
export async function showInstanceInFolder(instanceId) {
	const fullPath = await get_full_path(instanceId)
	return await openPath(fullPath)
}

export async function highlightModInInstance(instanceId, projectPath) {
	const fullPath = await get_mod_full_path(instanceId, projectPath)
	return await highlightInFolder(fullPath)
}

export async function restartApp() {
	return await apiClient.local.call(updates.restartApp)
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
