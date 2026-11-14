/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { logs } from '@modrinth/api-client'

import { apiClient } from '@/platform/modrinth-client'

/*
A log is a struct containing the filename string and optional output, as follows:

pub struct Logs {
    pub filename:  String,
    pub output: Option<String>,
}
*/

/// Get all logs that exist for a given instance
/// This is returned as an array of Log objects, sorted by filename (the folder name, when the log was created)
export async function get_logs(instanceId, clearContents) {
	return await apiClient.local.call(logs.ListInstanceLogs, {
		params: { id: instanceId },
		query: { clearContents },
	})
}

/// Get an instance's log text only by filename
export async function get_output_by_filename(instanceId, logType, filename) {
	return await apiClient.local.call(logs.GetInstanceLogOutput, {
		params: { id: instanceId, log_type: logType, filename },
	})
}

/// Delete an instance's log by filename
export async function delete_logs_by_filename(instanceId, logType, filename) {
	return await apiClient.local.call(logs.DeleteInstanceLog, {
		params: { id: instanceId, log_type: logType, filename },
	})
}

/// Get all buffered live log lines for an instance from the Rust ring buffer
export async function get_live_log_buffer(instanceId) {
	return await apiClient.local.call(logs.GetLiveLog, { params: { id: instanceId } })
}

/// Clear the live log buffer for an instance on the Rust side
export async function clear_log_buffer(instanceId) {
	return await apiClient.local.call(logs.ClearLiveLog, { params: { id: instanceId } })
}
