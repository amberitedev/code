export type {
	CoreInstance,
	CoreInstanceSummary,
	CoreInstanceStatus,
	CoreModLoader,
	CoreMemory,
	CoreStats,
	CoreMod,
	CoreFsEntry,
	CoreFsListing,
	CoreBackup,
	CoreBackupOperation,
	CoreBackupsResponse,
	CoreBackupSchedule,
	CoreCreateInstanceBody,
	CorePatchInstanceBody,
	CoreBackupScheduleBody,
	UploadHandle,
	CoreWsFrame,
	CoreSetupStatus,
	CoreSetupRequest,
	CoreSetupResponse,
} from './types'

export { CoreApiClient } from './client'
export { CoreWsConnection } from './ws'
export type { PlatformAdapter, PersistentQueueStore, QueuedMessage } from './adapter'
export type { CoreCallContext } from './context'
export {
	AmberiteApiError,
	NetworkError,
	AuthError,
	CoreOfflineError,
	RelayTimeoutError,
	CoreApiError,
} from './errors'
export { CoreConnectionMonitor, type ConnectionState } from './monitor'
export {
	publishMessage,
	pendingMessages,
	writeReceipt,
	writeResult,
	waitForReceipt,
	waitForResult,
	heartbeatCore,
	corePresence,
	messageDefinitions,
	type MessageEnvelope,
	type MessageDefinition,
	type MessageMode,
	type AckPolicy,
	type PublishOptions,
} from './transport'
export { pendingCoreRelayMessages, writeCoreRelayReceipt, writeCoreRelayResult } from './core-relay'
export { startMicrosoftLogin, completeMicrosoftLogin, type AuthSession } from './auth'
export { CoreHeartbeat } from './heartbeat'
