/**
 * Amberite API Service Layer
 * 
 * This file provides a unified interface for frontend calls.
 * All Tauri invoke() calls go through this service layer.
 * 
 * The naming convention:
 * - Theseus commands: `plugin:<module>|<command>` (e.g., `plugin:profile|profile_list`)
 * - Amberite commands: `plugin:amberite|<command>` (e.g., `plugin:amberite|amberite_hello`)
 */

import { invoke } from '@tauri-apps/api/core'

// ============================================================================
// AMBERITE COMMANDS
// ============================================================================

/**
 * Simple hello command to verify Amberite backend is working
 */
export async function amberiteHello(): Promise<string> {
	return await invoke('plugin:amberite|amberite_hello')
}

/**
 * Get Amberite version and placeholder data
 */
export async function amberiteGetVersion(): Promise<{ version: string; placeholder_message: string }> {
	return await invoke('plugin:amberite|amberite_get_version')
}

// ============================================================================
// THESEUS WRAPPERS - Profile Commands
// ============================================================================

/**
 * List all profiles/instances
 */
export async function profileList(): Promise<unknown[]> {
	return await invoke('plugin:profile|profile_list')
}

/**
 * Get a profile by path
 */
export async function profileGet(path: string): Promise<unknown | null> {
	return await invoke('plugin:profile|profile_get', { path })
}

/**
 * Create a new profile
 */
export async function profileCreate(
	name: string,
	gameVersion: string,
	modloader: string,
	loaderVersion: string | null,
	icon: string | null,
	skipInstall: boolean,
	linkedData?: { project_id: string; version_id: string; locked: boolean } | null
): Promise<string> {
	return await invoke('plugin:profile-create|profile_create', {
		name,
		gameVersion,
		modloader,
		loaderVersion,
		icon,
		skipInstall,
		linkedData,
	})
}

/**
 * Remove a profile
 */
export async function profileRemove(path: string): Promise<void> {
	return await invoke('plugin:profile|profile_remove', { path })
}

/**
 * Install/Repair a profile
 */
export async function profileInstall(path: string, force: boolean): Promise<void> {
	return await invoke('plugin:profile|profile_install', { path, force })
}

/**
 * Run a profile
 */
export async function profileRun(path: string, serverAddress?: string | null): Promise<unknown> {
	return await invoke('plugin:profile|profile_run', { path, serverAddress })
}

/**
 * Kill running profile
 */
export async function profileKill(path: string): Promise<void> {
	return await invoke('plugin:profile|profile_kill', { path })
}

// ============================================================================
// THESEUS WRAPPERS - Settings Commands
// ============================================================================

/**
 * Get application settings
 */
export async function settingsGet(): Promise<unknown> {
	return await invoke('plugin:settings|settings_get')
}

/**
 * Set application settings
 */
export async function settingsSet(settings: unknown): Promise<void> {
	return await invoke('plugin:settings|settings_set', { settings })
}

// ============================================================================
// THESEUS WRAPPERS - Auth Commands
// ============================================================================

/**
 * Get Modrinth credentials
 */
export async function authCredentials(): Promise<unknown | null> {
	return await invoke('plugin:auth|auth_credentials')
}

/**
 * Login to Modrinth
 */
export async function authLogin(): Promise<void> {
	return await invoke('plugin:auth|auth_login')
}

/**
 * Logout from Modrinth
 */
export async function authLogout(): Promise<void> {
	return await invoke('plugin:auth|auth_logout')
}

// ============================================================================
// APP LIFECYCLE
// ============================================================================

/**
 * Initialize the app state (called on startup)
 */
export async function initializeState(): Promise<void> {
	return await invoke('initialize_state')
}

/**
 * Show the main window
 */
export function showWindow(): void {
	return invoke('show_window')
}

/**
 * Check if running in dev mode
 */
export function isDev(): Promise<boolean> {
	return invoke('is_dev')
}

/**
 * Restart the application
 */
export function restartApp(): void {
	return invoke('restart_app')
}

// ============================================================================
// API STATUS
// ============================================================================

/**
 * Test that both Theseus and Amberite backends are accessible
 */
export async function testBackends(): Promise<{
	theseus: { status: 'ok' | 'error'; message: string }
	amberite: { status: 'ok' | 'error'; version: string; message: string }
}> {
	const result = {
		theseus: { status: 'ok' as const, message: '' },
		amberite: { status: 'ok' as const, version: '', message: '' },
	}

	// Test Theseus
	try {
		await profileList()
		result.theseus.message = 'Profile list accessible'
	} catch (e) {
		result.theseus.status = 'error'
		result.theseus.message = String(e)
	}

	// Test Amberite
	try {
		const versionInfo = await amberiteGetVersion()
		result.amberite.version = versionInfo.version
		result.amberite.message = versionInfo.placeholder_message
	} catch (e) {
		result.amberite.status = 'error'
		result.amberite.message = String(e)
	}

	return result
}
