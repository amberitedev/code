import {
	type AbstractModrinthClient,
	AuthFeature,
	GenericModrinthClient,
	NodeAuthFeature,
	nodeAuthState,
	PanelVersionFeature,
	SelfHostedFeature,
	TauriModrinthClient,
	VerboseLoggingFeature,
} from '@modrinth/api-client'
import { invoke } from '@tauri-apps/api/core'

import { config } from '@/config'
import { get as getModrinthCredentials } from '@/helpers/mr_auth'
import { createLocalFileUploader } from '@/platform/adapters/local-file-upload'
import { isNativeWindow, localTransport } from '@/platform/local'
import { getVersion } from '@/platform/os'

/** The account signed in. A Core session belongs to one account and one Core. */
async function account(): Promise<string> {
	const credentials = await getModrinthCredentials()
	if (!credentials) throw new Error('Sign in to use a Core')
	return credentials.user_id
}

/**
 * The App's one client for Modrinth, the account backend, Cores and its own local server. Root
 * setup provides it to components; code outside components imports it directly. A tab has no
 * Tauri HTTP plugin, so it uses `fetch`.
 */
const ModrinthClient = isNativeWindow ? TauriModrinthClient : GenericModrinthClient
export const apiClient: AbstractModrinthClient = new ModrinthClient({
	userAgent: async () => `modrinth/theseus/${await getVersion()}`,
	labrinthBaseUrl: config.labrinthBaseUrl,
	// With our own account backend, hosting is the account's own Cores, found through it.
	selfHostedHosting: Boolean(config.accountApiUrl),
	// A Core's own certificate is checked in Rust; the WebView talks to a local proxy instead,
	// which adds this device's session for the account signed in. The WebView never holds it.
	connectCore: async (coreId, address, fingerprint) =>
		invoke<string>('plugin:cores|core_connect', {
			coreId,
			account: await account(),
			address,
			fingerprint,
		}),
	coreSessions: {
		signIn: async (coreId, address, fingerprint, ticket) =>
			invoke('plugin:cores|core_sign_in', {
				coreId,
				account: await account(),
				address,
				fingerprint,
				ticket,
			}),
		signOut: async (coreId) =>
			invoke('plugin:cores|core_sign_out', { coreId, account: await account() }),
		exists: async (coreId) =>
			invoke('plugin:cores|core_session_exists', { coreId, account: await account() }),
	},
	archonBaseUrl: config.archonBaseUrl,
	sharedInstancesBaseUrl: config.sharedInstancesBaseUrl,
	localFileUploader: createLocalFileUploader(),
	localTransport,
	features: [
		...(config.accountApiUrl
			? [
					new SelfHostedFeature({
						baseUrl: config.accountApiUrl,
						token: async () => (await getModrinthCredentials())?.session,
					}),
				]
			: []),
		new NodeAuthFeature({
			getAuth: () => nodeAuthState.getAuth?.() ?? null,
			refreshAuth: async () => await nodeAuthState.refreshAuth?.(),
		}),
		new AuthFeature({
			token: async () => (await getModrinthCredentials())?.session,
		}),
		new PanelVersionFeature(),
		new VerboseLoggingFeature(),
	],
})
