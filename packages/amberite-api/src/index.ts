export type {
	CoreInstance,
	CoreInstanceSummary,
	CoreInstanceStatus,
	CoreInstanceInstallStatus,
	CoreInstanceEvent,
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
	CoreModpackManifest,
	FsDownloadUrlResponse,
	UnzipOption,
	FsZipRequest,
	FsCopyRequest,
	CoreFsOperationKind,
	CoreMetadata,
	CoreMember,
	CoreSyncProfile,
} from './types'

export type {
	AmberiteUser,
	CorePresence,
	FriendGroupInfo,
	FriendGroupSummary,
	FriendGroupMember,
	FriendGroupInvite,
	ConvexSyncedProfile,
} from './convex-types'

export { CoreApiClient, CoreEventStream } from './client'
export { ConvexApiClient } from './convex-api'
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
export { CoreInstanceStateManager, type CoreInstanceStateSnapshot } from './instance-state'
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
export {
	pendingCoreRelayMessages,
	writeCoreRelayReceipt,
	writeCoreRelayResult,
	coreRelayMessageStatus,
	waitForCoreRelayReceipt,
	waitForCoreRelayResult,
} from './core-relay'
export { CommunicationPipeline, type CommunicationPipelineOptions } from './pipeline'
export {
	defaultCommunicationPolicy,
	coreEndpointPolicies,
	resolveCoreEndpointKey,
	mergePolicy,
} from './endpoint-policies'
export { MemoryQueueStore, CompositeQueueStore } from './queue'
export type {
	CommunicationSurface,
	CommunicationMethod,
	CommunicationReliability,
	CommunicationAuthMode,
	CommunicationNode,
	CommunicationPolicy,
	CommunicationPipelineOptions,
	CommunicationPolicyOverride,
	CommunicationCall,
	CommunicationPublish,
	CommunicationResult,
	EndpointPolicyMap,
} from './pipeline-types'
export { startMicrosoftLogin, completeMicrosoftLogin, type AuthSession } from './auth'
export { CoreHeartbeat } from './heartbeat'
export { drainQueue } from './drain'
