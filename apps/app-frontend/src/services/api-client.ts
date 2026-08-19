import {
	AmberiteFeature,
	AuthFeature,
	NodeAuthFeature,
	nodeAuthState,
	PanelVersionFeature,
	TauriModrinthClient,
	VerboseLoggingFeature,
	amberiteFeatureConfig,
} from '@modrinth/api-client'
import { getVersion } from '@tauri-apps/api/app'

import { config } from '@/config'
import { get as getModrinthCredentials } from '@/helpers/mr_auth'
import { amberite } from '@/services/amberite'

const appVersion = getVersion()

export const apiClient = new TauriModrinthClient({
	userAgent: async () => `amberite/${await appVersion}`,
	labrinthBaseUrl: config.labrinthBaseUrl,
	archonBaseUrl: config.archonBaseUrl,
	sharedInstancesBaseUrl: config.sharedInstancesBaseUrl,
	features: [
		new NodeAuthFeature({
			getAuth: () => nodeAuthState.getAuth?.() ?? null,
			refreshAuth: async () => await nodeAuthState.refreshAuth?.(),
		}),
		new AuthFeature({
			token: async () => (await getModrinthCredentials())?.session,
		}),
		new PanelVersionFeature(),
		new VerboseLoggingFeature(),
		new AmberiteFeature(amberiteFeatureConfig(amberite.adapter, amberite.transport)),
	],
})

export { appVersion }
