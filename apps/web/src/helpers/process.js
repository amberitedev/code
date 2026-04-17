/**
 * Mocked process helper - returns empty/fake data
 * TODO: connect to backend API - replace with real API calls
 */

export async function get_by_profile_path(_path) {
	return []
}

export async function get_all() {
	return []
}

export async function kill(_uuid) {
	// no-op
}
