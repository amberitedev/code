/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { processes } from '@modrinth/api-client'

import { apiClient } from '@/platform/modrinth-client'

/// Gets all running process IDs with a given instance ID
/// Returns [u32]
export async function get_by_instance_id(instanceId) {
	return await apiClient.local.call(processes.ListInstanceProcesses, {
		params: { id: instanceId },
	})
}

/// Gets all running process IDs
/// Returns [u32]
export async function get_all() {
	return await apiClient.local.call(processes.ListProcesses)
}

/// Kills a process by UUID
export async function kill(uuid) {
	return await apiClient.local.call(processes.KillProcess, { params: { uuid } })
}
