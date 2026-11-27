/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { logs } from '@amberite/contracts/engine'

import { apiClient } from '@/platform/modrinth-client'

/// Get all buffered live log lines for an instance from the Rust ring buffer
export async function get_live_log_buffer(instanceId) {
	return await apiClient.local.call(logs.GetLiveLog, { params: { id: instanceId } })
}

/// Clear the live log buffer for an instance on the Rust side
export async function clear_log_buffer(instanceId) {
	return await apiClient.local.call(logs.ClearLiveLog, { params: { id: instanceId } })
}
