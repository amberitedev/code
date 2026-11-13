/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { type Settings, settings as settingsRoutes } from '@modrinth/api-client'
import { queryOptions } from '@tanstack/vue-query'

import { apiClient } from '@/platform/modrinth-client'

// Settings object
/*

Settings {
    "memory": MemorySettings,
    "game_resolution": [int int],
    "custom_java_args": [String ...],
    "custom_env_args" : [(string, string) ... ]>,
    "java_globals": Hash of (string, Path),
    "default_user": Uuid string (can be null),
    "hooks": Hooks,
    "max_concurrent_downloads": uint,
    "version": u32,
    "collapsed_navigation": bool,
}

Memorysettings {
    "min": u32, can be null,
    "max": u32,
}

*/

export type AppSettings = Settings

export const appSettingsKeys = {
	all: ['app-settings'] as const,
	update: ['app-settings', 'update'] as const,
}

export function appSettingsQueryOptions() {
	return queryOptions({
		queryKey: appSettingsKeys.all,
		networkMode: 'always',
		queryFn: get,
		staleTime: 0,
	})
}

export function serializeEnvVars(vars: [string, string][] | undefined | null): string {
	return (vars ?? []).map(([key, value]) => `${key}=${value}`).join(' ')
}

export function parseEnvVars(input: string | undefined | null): [string, string][] {
	if (!input?.trim()) {
		return []
	}

	const vars: [string, string][] = []
	for (const entry of input.trim().split(/\s+/)) {
		const separator = entry.indexOf('=')
		if (separator <= 0) {
			continue
		}
		vars.push([entry.slice(0, separator), entry.slice(separator + 1)])
	}
	return vars
}

// Get full settings object
export async function get() {
	return await apiClient.local.call(settingsRoutes.getSettings)
}

// Set full settings object
export async function set(settings: AppSettings) {
	return await apiClient.local.call(settingsRoutes.setSettings, { body: settings })
}

export async function cancel_directory_change(): Promise<void> {
	return await apiClient.local.call(settingsRoutes.cancelDirectoryChange)
}
