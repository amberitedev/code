/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import {
	localAccounts,
	type ModrinthAuthFlow,
	type ModrinthCredentials,
} from '@modrinth/api-client'

import { apiClient } from '@/platform/modrinth-client'

export type { ModrinthAuthFlow, ModrinthCredentials }

export async function login(
	flow: ModrinthAuthFlow = 'sign-in',
	addAccount = false,
): Promise<ModrinthCredentials> {
	return await apiClient.local.call(localAccounts.signInToModrinth, {
		body: { flow, add_account: addAccount },
	})
}

export async function logout(): Promise<void> {
	return await apiClient.local.call(localAccounts.signOutOfModrinth)
}

export async function get(): Promise<ModrinthCredentials | null> {
	return await apiClient.local.call(localAccounts.getActiveModrinthAccount)
}

export async function getAll(): Promise<ModrinthCredentials[]> {
	return await apiClient.local.call(localAccounts.listModrinthAccounts)
}

export async function setActive(userId: string): Promise<void> {
	return await apiClient.local.call(localAccounts.setActiveModrinthAccount, { body: userId })
}

export async function removeUser(userId: string): Promise<void> {
	return await apiClient.local.call(localAccounts.removeModrinthAccount, { params: { id: userId } })
}

export async function cancelLogin(): Promise<void> {
	return await apiClient.local.call(localAccounts.cancelModrinthSignIn)
}
