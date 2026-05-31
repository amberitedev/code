import { ListIcon, SettingsIcon, TextQuoteIcon, VersionIcon, WrenchIcon } from '@modrinth/assets'
import type { Component } from 'vue'

import type { CoreServerViewData } from '../core-server-instance'
import ServerSettingsAdvanced from './pages/ServerSettingsAdvanced.vue'
import ServerSettingsGeneral from './pages/ServerSettingsGeneral.vue'
import ServerSettingsInstallation from './pages/ServerSettingsInstallation.vue'
import ServerSettingsNetwork from './pages/ServerSettingsNetwork.vue'
import ServerSettingsProperties from './pages/ServerSettingsProperties.vue'

export type ServerSettingsTabId = 'general' | 'installation' | 'network' | 'properties' | 'advanced'

export interface ServerSettingsTab {
	id: ServerSettingsTabId
	label: string
	icon: Component
	component: Component
	/** Hide the tab while the instance is still installing. */
	shown?: (status: CoreServerViewData['status']) => boolean
}

export const serverSettingsTabs: ServerSettingsTab[] = [
	{ id: 'general', label: 'General', icon: SettingsIcon, component: ServerSettingsGeneral },
	{
		id: 'installation',
		label: 'Installation',
		icon: WrenchIcon,
		component: ServerSettingsInstallation,
	},
	{ id: 'network', label: 'Network', icon: VersionIcon, component: ServerSettingsNetwork },
	{
		id: 'properties',
		label: 'Properties',
		icon: ListIcon,
		component: ServerSettingsProperties,
		shown: (status) => status !== 'installing',
	},
	{ id: 'advanced', label: 'Advanced', icon: TextQuoteIcon, component: ServerSettingsAdvanced },
]
