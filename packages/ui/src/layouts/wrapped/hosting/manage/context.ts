import type {
	CoreChangeVersionBody,
	CoreInstance,
	CoreStats,
} from '@amberite/amberite-api'
import type { Archon } from '@modrinth/api-client'
import type { Stats } from '@modrinth/utils'
import type { ComputedRef, InjectionKey, Ref } from 'vue'
import { inject } from 'vue'

import type { LogLine } from '#ui/layouts/shared/console/types'

export type CoreServerViewData = Archon.Servers.v0.Server

export interface CoreServerContext {
	instanceId: ComputedRef<string>
	rawInstance: Ref<CoreInstance | null>
	server: ComputedRef<CoreServerViewData | null>
	statsData: Ref<CoreStats | null>
	stats: Ref<Stats>
	powerState: Ref<Archon.Websocket.v0.PowerState>
	logLines: Ref<LogLine[]>
	refreshServer: () => Promise<void>
	refreshStats: () => Promise<void>
	sendCommand: (command: string) => Promise<void>
	startServer: () => Promise<void>
	stopServer: () => Promise<void>
	restartServer: () => Promise<void>
	killServer: () => Promise<void>
	repairServer: () => Promise<void>
	changeVersion: (body: CoreChangeVersionBody) => Promise<void>
	copyId: () => Promise<void>
}

export const coreServerContextKey = Symbol('core-server-context') as InjectionKey<CoreServerContext>

export function injectCoreServerContext(): CoreServerContext {
	const context = inject(coreServerContextKey)
	if (!context) {
		throw new Error('CoreServerContext not provided')
	}
	return context
}
