/**
 * Mocked mr_auth helper - returns null
 * TODO: connect to backend API - replace with real authentication
 */

export type ModrinthCredentials = {
	session: string
	expires: string
	user_id: string
	active: boolean
}

export async function login(): Promise<ModrinthCredentials> {
	return {
		session: '',
		expires: '',
		user_id: '',
		active: false,
	}
}

export async function logout(): Promise<void> {
	// no-op
}

export async function get(): Promise<ModrinthCredentials | null> {
	return null
}

export async function cancelLogin(): Promise<void> {
	// no-op
}
