import type { Ref } from 'vue'

import { createContext } from '#ui/providers/create-context'

export interface ServerSettingsBrowseModpacksArgs {
	serverId: string
	from: 'reset-server'
}

export interface ServerSettingsContext {
	siteUrl: Ref<string>
	isApp: Ref<boolean>
	browseModpacks: (args: ServerSettingsBrowseModpacksArgs) => void | Promise<void>
	closeModal?: () => void
	saveBanner?: {
		target: Ref<HTMLElement | null>
		shown: Ref<boolean>
	}
}

export const [injectServerSettings, provideServerSettings] = createContext<ServerSettingsContext>(
	'ServerSettings',
	'serverSettingsContext',
)
