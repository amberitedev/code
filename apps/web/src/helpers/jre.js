/**
 * Mocked jre helper - returns empty/fake data
 * TODO: connect to backend API - replace with real API calls
 */

export async function get_java_versions() {
	return []
}

export async function set_java_version(javaVersion) {
	// no-op
}

export async function find_filtered_jres(version) {
	return []
}

export async function get_jre(path) {
	return null
}

export async function test_jre(path, majorVersion) {
	return false
}

export async function auto_install_java(javaVersion) {
	// no-op
}

export async function get_max_memory() {
	return 4096
}
