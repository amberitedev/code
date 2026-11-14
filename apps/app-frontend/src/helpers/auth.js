/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { ApiCallError, localAccounts } from '@modrinth/api-client'

import { apiClient } from '@/platform/modrinth-client'

// Example function:
// User goes to auth_url to complete flow, and when completed, authenticate_await_completion() returns the credentials
// export async function authenticate() {
//   const auth_url = await authenticate_begin_flow()
//   console.log(auth_url)
//   await authenticate_await_completion()
// }

/**
 * Check if the authentication servers are reachable, throwing an exception if
 * not reachable.
 */
export async function check_reachable() {
	await apiClient.local.call(localAccounts.checkMinecraftReachable)
}

/**
 * Authenticate a user with Hydra - part 1.
 * This begins the authentication flow quasi-synchronously.
 *
 * @returns {Promise<DeviceLoginSuccess>} A DeviceLoginSuccess object with two relevant fields:
 * @property {string} verification_uri - The URL to go to complete the flow.
 * @property {string} user_code - The code to enter on the verification_uri page.
 */
export async function login() {
	try {
		return await apiClient.local.call(localAccounts.signInToMinecraft)
	} catch (error) {
		if (error instanceof ApiCallError && error.code === 'sign_in_cancelled') return null
		throw error
	}
}

/**
 * Retrieves the default user
 * @return {Promise<UUID | undefined>}
 */
export async function get_default_user() {
	return await apiClient.local.call(localAccounts.getDefaultMinecraftAccount)
}

/**
 * Updates the default user
 * @param {UUID} user
 */
export async function set_default_user(user) {
	return await apiClient.local.call(localAccounts.setDefaultMinecraftAccount, { body: user })
}

/**
 * Remove a user account from the database
 * @param {UUID} user
 */
export async function remove_user(user) {
	return await apiClient.local.call(localAccounts.removeMinecraftAccount, { params: { id: user } })
}

/**
 * Returns a list of users
 * @returns {Promise<Array<{profile: {id: string, name: string}, expires: string, active: boolean}>>}
 */
export async function users() {
	return await apiClient.local.call(localAccounts.listMinecraftAccounts)
}
