import { ContractClient, desktop } from '@modrinth/api-client'
import { invoke } from '@tauri-apps/api/core'

import { get_full_path, get_mod_full_path } from '@/helpers/instance'
import { save } from '@/platform/dialog'
import { localTransport } from '@/platform/local'
import { getBoot } from '@/platform/os'

const local = new ContractClient(localTransport)

export async function isDev() {
	return getBoot().dev
}

export async function areUpdatesEnabled() {
	return getBoot().updates_enabled
}

export async function getUpdateSize(updateRid) {
	return await invoke('get_update_size', { rid: updateRid })
}

export async function enqueueUpdateForInstallation(updateRid) {
	return await invoke('enqueue_update_for_installation', { rid: updateRid })
}

export async function removeEnqueuedUpdate() {
	return await invoke('remove_enqueued_update')
}

export async function setRestartAfterPendingUpdate(should_restart) {
	return await invoke('set_restart_after_pending_update', { shouldRestart: should_restart })
}

// One of 'Windows', 'Linux', 'MacOS'
export async function getOS() {
	return { windows: 'Windows', macos: 'MacOS', linux: 'Linux' }[getBoot().platform]
}

export async function isNetworkMetered() {
	return await local.call(desktop.checkNetworkMetered)
}

export async function openPath(path) {
	return await local.call(desktop.openPath, { body: { path } })
}

export async function highlightInFolder(path) {
	return await local.call(desktop.revealPath, { body: { path } })
}

export async function showLauncherLogsFolder() {
	return await local.call(desktop.openFolder, { body: { folder: 'launcher_logs' } })
}

export async function createInstanceShortcut(instanceName, instanceId, options = {}) {
	const outputPath = await save({
		defaultPath: `Modrinth - ${instanceName}`,
	})

	if (!outputPath) return null

	return await local.call(desktop.createShortcut, {
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
	return await local.call(desktop.openFolder, { body: { folder: 'app_db_backups' } })
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
	return await invoke('restart_app')
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
