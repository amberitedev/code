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
} from './types'

export { CoreApiClient } from './client'
export { CoreWsConnection } from './ws'
export type { PlatformAdapter } from './adapter'
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
	writeReceipt,
	writeResult,
	waitForReceipt,
	waitForResult,
	subscribeToMessages,
	type RelayMessage,
	type MessageDirection,
	type PublishOptions,
} from './transport'
export { startMicrosoftLogin, completeMicrosoftLogin, type AuthSession } from './auth'
export { CoreHeartbeat } from './heartbeat'
