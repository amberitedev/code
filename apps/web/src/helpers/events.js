/**
 * Mocked events helper - returns no-op listeners
 * TODO: connect to backend API - replace with real API calls
 */

export async function loading_listener(_callback) {
	return () => {}
}

export async function process_listener(_callback) {
	return () => {}
}

export async function profile_listener(_callback) {
	return () => {}
}

export async function command_listener(_callback) {
	return () => {}
}

export async function warning_listener(_callback) {
	return () => {}
}

export async function friend_listener(_callback) {
	return () => {}
}
