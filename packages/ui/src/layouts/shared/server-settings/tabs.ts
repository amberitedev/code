import type { Archon } from '@modrinth/api-client'
import {
	DatabaseBackupIcon,
	ListIcon,
	SettingsIcon,
	TextQuoteIcon,
	UsersIcon,
	WrenchIcon,
} from '@modrinth/assets'
import type { Component } from 'vue'

export type ServerSettingsTabId =
	| 'general'
	| 'installation'
	| 'sharing'
	| 'backups'
	| 'properties'
	| 'advanced'

export interface ServerSettingsTabContext {
	serverStatus?: Archon.Servers.v0.Status | null
	isShared: boolean
}

export interface ServerSettingsTabDefinition {
	id: ServerSettingsTabId
	label: string
	icon: Component
	shown?: (ctx: ServerSettingsTabContext) => boolean
}

export const serverSettingsTabDefinitions: ServerSettingsTabDefinition[] = [
	{
		id: 'general',
		label: 'General',
		icon: SettingsIcon,
	},
	{
		id: 'installation',
		label: 'Installation',
		icon: WrenchIcon,
	},
	{
		id: 'sharing',
		label: 'Sharing',
		icon: UsersIcon,
		shown: ({ isShared }) => isShared,
	},
	{
		id: 'backups',
		label: 'Backups',
		icon: DatabaseBackupIcon,
	},
	{
		id: 'properties',
		label: 'Properties',
		icon: ListIcon,
		shown: ({ serverStatus }) => serverStatus !== 'installing',
	},
	{
		id: 'advanced',
		label: 'Advanced',
		icon: TextQuoteIcon,
	},
]
