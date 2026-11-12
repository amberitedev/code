/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { ApiCallError, type Command, events, loadingBars } from '@modrinth/api-client'
import { invoke } from '@tauri-apps/api/core'

import { apiClient } from '@/platform/modrinth-client'

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

// Resolves once the engine is up: the App opens the event socket only then.
export async function initialize_state(openEvents: () => Promise<void>) {
	try {
		await openEvents()
	} catch (error) {
		// A page cannot read why a socket was refused; over HTTP the same route says why the engine
		// did not start.
		const refusal = await apiClient.local
			.call(events.eventSocket)
			.catch((refusal: unknown) => refusal)
		throw refusal instanceof ApiCallError && refusal.code === 'engine_unavailable' ? refusal : error
	}
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
	return await invoke<Command | null>('plugin:utils|get_opening_command')
}
