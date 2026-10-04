import {
	AuthFeature,
	NodeAuthFeature,
	nodeAuthState,
	PanelVersionFeature,
	TauriModrinthClient,
	VerboseLoggingFeature,
	SelfHostedFeature,
} from '@modrinth/api-client'
import { getVersion } from '@tauri-apps/api/app'

import { config } from '@/config'
import { get as getModrinthCredentials } from '@/helpers/mr_auth'

const appVersion = getVersion()

export const apiClient = new TauriModrinthClient({
	userAgent: async () => `modrinth/theseus/${await appVersion}`,
	labrinthBaseUrl: config.labrinthBaseUrl,
	selfHostedHosting: Boolean(config.coreUrl),
	archonBaseUrl: () => (config.coreUrl ? `${config.coreUrl}/hosting` : config.archonBaseUrl),
	sharedInstancesBaseUrl: config.sharedInstancesBaseUrl,
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

export { appVersion }
