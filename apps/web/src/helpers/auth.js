/**
 * Mocked auth helper - returns empty/fake data
 * TODO: connect to backend API - replace with real API calls
 */

export async function check_reachable() {
	// no-op - always reachable in web mode
}

export async function login() {
	return null
}

export async function get_default_user() {
	return null
}

export async function set_default_user(_user) {
	// no-op
}

export async function remove_user(_user) {
	// no-op
}

export async function users() {
	return []
}
