/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { ApiCallError, desktop, events, loadingBars } from '@modrinth/api-client'

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
		// A page cannot read why a socket was refused, so it asks the same route over HTTP. A failed
		// start answers with its code; a started engine, with axum's uncoded upgrade rejection, and
		// then the handshake only timed out during a long start.
		const refusal = await apiClient.local
			.call(events.eventSocket)
			.catch((refusal: unknown) => refusal)
		if (!(refusal instanceof ApiCallError)) throw error
		if (refusal.code !== 'unknown') throw refusal
		await openEvents()
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
	return await apiClient.local.call(desktop.getOpeningCommand)
}
