import type { ContractClient, PowerState, Server, UploadState } from '@modrinth/api-client'
import type { ComputedRef, Ref } from 'vue'

import type { MessageDescriptor } from '#ui/composables/i18n'
import type {
	ServerInstallationKey,
	ServerInstallationState,
} from '#ui/composables/server-installation-tracker'
import type { FileOperation } from '#ui/layouts/shared/files-tab/types'

import { createContext } from '.'

export interface BusyReason {
	reason: MessageDescriptor
}

export type CancelUploadHandler = () => void | Promise<void>

/** The server a panel shows, on its Core. */
export interface ModrinthServerContext {
	readonly serverId: string
	/** The Core that has the server, and its routes. */
	readonly coreId: string
	readonly core: ContractClient
	readonly server: Ref<Server>

	/** The Core's event socket is open. */
	readonly isConnected: Ref<boolean>
	readonly powerState: Ref<PowerState>
	readonly isServerRunning: ComputedRef<boolean>
	readonly uptimeSeconds: Ref<number>
	readonly installation: ComputedRef<ServerInstallationState | null>
	beginInstallation: (key: ServerInstallationKey) => void
	cancelOptimisticInstallation: () => void
	dismissInstallation: (id: string) => void

	// Busy state — when non-empty, all write operations should be disabled
	readonly busyReasons: ComputedRef<BusyReason[]>

	// File upload state
	readonly uploadState: Ref<UploadState>
	readonly cancelUpload: Ref<CancelUploadHandler | null>

	// File operations this page runs, such as zipping
	readonly activeOperations: ComputedRef<FileOperation[]>
	dismissOperation: (opId: string, action: 'dismiss' | 'cancel') => Promise<void>
	upsertLocalFileOperation?: (operation: FileOperation) => void
}

export const [injectModrinthServerContext, provideModrinthServerContext] =
	createContext<ModrinthServerContext>('[id].vue', 'modrinthServerContext')
