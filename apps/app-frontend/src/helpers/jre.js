/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { desktop, java } from '@modrinth/api-client'
import { queryOptions } from '@tanstack/vue-query'

import { apiClient } from '@/platform/modrinth-client'

/*

JavaVersion {
    path: Path
    version: String
}

*/

export async function get_java_versions() {
	return await apiClient.local.call(java.GetJavaVersions)
}

// A path typed into an empty slot comes without its version, so it is saved once it names a Java.
// An empty path saves nothing: a slot cannot be cleared.
export async function set_java_version(javaVersion) {
	if (!javaVersion?.path) return
	let body = javaVersion
	if (javaVersion.parsed_version === undefined) {
		body = await get_jre(javaVersion.path).catch(() => null)
		if (!body) return
	}
	return await apiClient.local.call(java.SetJavaVersion, { body })
}

// Finds all the installation of Java 7, if it exists
// Returns [JavaVersion]
export async function find_filtered_jres(version) {
	return await apiClient.local.call(java.FindJavaInstalls, { query: { version } })
}

// Gets java version from a specific path by trying to run 'java -version' on it.
// This also validates it, as it returns null if no valid java version is found at the path
export async function get_jre(path) {
	return await apiClient.local.call(desktop.getJavaAt, { query: { path } })
}

// Tests JRE version by running 'java -version' on it.
// Returns true if the version is valid, and matches given (after extraction)
export async function test_jre(path, majorVersion) {
	return await apiClient.local.call(desktop.testJavaAt, {
		query: { path, major_version: majorVersion },
	})
}

// Automatically installs specified java version
export async function auto_install_java(javaVersion) {
	return await apiClient.local.call(java.InstallJava, { body: javaVersion })
}

// Get max memory in KiB
export async function get_max_memory() {
	return await apiClient.local.call(java.GetSystemMemory)
}

export function maxMemoryQueryOptions() {
	return queryOptions({
		queryKey: ['system', 'memory', 'total'],
		networkMode: 'always',
		queryFn: get_max_memory,
		staleTime: Infinity,
		gcTime: Infinity,
	})
}
