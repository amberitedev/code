/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { ApiCallError, desktop, loadingBars } from '@modrinth/api-client'

import { apiClient } from '@/platform/modrinth-client'
import type { AppEventPayload, AppEvents } from '@/providers/app-events'

export interface LoadingBarType {
	type?: string
	version?: string | number
	instance_id?: string
	instance_name?: string
	pack_name?: string
	icon?: string | null
}

export interface LoadingBar {
	id?: string | number
	loading_bar_uuid?: string | number
	title?: string
	message?: string
	current?: number
	total?: number
	bar_type?: LoadingBarType
}

// Resolves once the engine is up. The event socket opens at once and reports how the start ended.
export async function initialize_state(appEvents: AppEvents, openEvents: () => Promise<void>) {
	const started = new Promise<AppEventPayload<'engine_started'>>((resolve) =>
		appEvents.once('engine_started', resolve),
	)
	await openEvents()
	const { error } = await started
	if (error) throw new ApiCallError(503, error.error, error.description)
}

// Gets active progress bars
export async function progress_bars_list(): Promise<Record<string, LoadingBar>> {
	return await apiClient.local.call(loadingBars.ListLoadingBars)
}

// Get opening command
// For example, if a user clicks on an .mrpack to open the app.
// This should be called once and only when the app is done booting up and ready to receive a command
// Returns a Command struct- see events.js
export async function get_opening_command() {
	return await apiClient.local.call(desktop.getOpeningCommand)
}
