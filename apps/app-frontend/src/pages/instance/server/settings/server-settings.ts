import { createContext } from '@modrinth/ui'
import type { Ref } from 'vue'

export interface ServerSettingsBrowseModpacksArgs {
	serverId: string
	worldId: string | null
	from: 'reset-server'
}

export interface ServerSettingsContext {
	isApp: Ref<boolean>
	currentUserId: Ref<string | null>
	currentUserRole: Ref<string | null>
	browseModpacks: (args: ServerSettingsBrowseModpacksArgs) => void | Promise<void>
	closeModal?: () => void
}

export const [injectServerSettings, provideServerSettings] = createContext<ServerSettingsContext>(
	'ServerSettings',
	'serverSettingsContext',
)
