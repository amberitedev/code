export type {
	CoreInstance,
	CoreInstanceSummary,
	CoreInstanceStatus,
	CoreInstanceInstallStatus,
	CoreInstanceEvent,
	CoreModLoader,
	CoreMemory,
	CoreStats,
	CorePlayers,
	CoreWhitelistEntry,
	CoreOpEntry,
	CoreBanEntry,
	CoreIpBanEntry,
	CoreRconEnableResult,
	CoreServerQuery,
	CoreScheduledTask,
	CoreCreateTaskBody,
	CoreUpdateTaskBody,
	CoreTaskType,
	CoreMod,
	CoreFsEntry,
	CoreFsListing,
	CoreBackup,
	CoreBackupOperation,
	CoreBackupsResponse,
	CoreBackupSchedule,
	CoreCreateInstanceBody,
	CorePatchInstanceBody,
	CoreChangeVersionBody,
	CoreStartupSettings,
	CoreBackupScheduleBody,
	UploadHandle,
	CoreWsFrame,
	CoreSetupStatus,
	CoreSetupRequest,
	CoreSetupResponse,
	CoreConnectionHandshakeRequest,
	CoreConnectionHandshakeResponse,
	CoreConnectionRejectReason,
	CoreModpackManifest,
	FsDownloadUrlResponse,
	UnzipOption,
	FsZipRequest,
	FsCopyRequest,
	CoreFsOperationKind,
	CoreMetadata,
	CoreProjectionSyncResult,
	CoreMember,
	CoreAccessRole,
	CorePermissionPreset,
	CoreAccessMember,
	CoreAccessViewer,
	CoreAccessResponse,
	CoreAccessUpsertBody,
	CoreAccessPatchBody,
	CoreRole,
	CoreRoleConfiguration,
	SaveCoreRoleBody,
	CoreInvitation,
	CoreInvitationStatus,
	CreateCoreInvitationBody,
	CoreActivityLogEntry,
	CoreActivityLogQuery,
	CoreActivityLogResponse,
	CoreSyncProfile,
} from './types'

export type {
	AmberiteUser,
	AmberiteProfile,
	AmberitePublicProfile,
	LinkedModrinthAccount,
	CoreListEntry,
	CorePresence,
	FriendGroupInfo,
	FriendGroupSummary,
	FriendGroupMember,
	FriendGroupInvite,
	FriendGroupBan,
	ConvexSyncedProfile,
	ConvexProfileSnapshot,
	ConvexModSyncEvent,
	ProfileVisibility,
	WhitelistScope,
	FriendGroupRoleName,
	SyncedProfileSettings,
	ProfileWhitelistEntry,
	ProfileWhitelistResult,
} from './convex-types'

export { CoreApiClient, CoreEventStream } from './client'
export { ConvexApiClient } from './convex-api'
export type {
	AmberiteSocialClient,
	FriendsListResult,
	FriendRequestEntry,
	GroupInviteWithGroup,
} from './convex-api'
export { ConvexAmberiteAuthClient } from './auth-client'
export type {
	AmberiteAuthClient,
	AmberiteSession,
	AmberitePasswordSignInRequest,
	AmberitePasswordSignUpRequest,
	AmberiteMinecraftTokenSignInRequest,
} from './auth-client'
export { MockAmberiteAuthClient, defaultMockUser } from './mock-client'
export type { MockAmberiteAuthClientOptions } from './mock-client'
export {
	adapterSessionStorage,
	createMemoryAmberiteSessionStorage,
	isAmberiteSessionTokens,
	validateAmberiteSessionTokens,
} from './session'
export type { AmberiteSessionTokens, AmberiteSessionStorage } from './session'
export {
	mapAmberiteUserToAccountUser,
	normalizeAmberiteAccountUser,
} from './profile'
export type { AmberiteAccountUser, AmberiteProfilePatch } from './profile'
export { composeSocialSessionState } from './social-session'
export type {
	DurableSocialSessionState,
	LiveSocialState,
	LiveUserState,
	SocialSessionState,
} from './social-session'
export { CoreWsConnection } from './ws'
export { RealtimePresenceSession } from './realtime'
export type {
	RealtimeFrame,
	RealtimePresenceSessionOptions,
	RealtimeSocket,
} from './realtime'
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
export { CoreConnectionMonitor, type ConnectionState, type ConnectionStatus } from './monitor'
export {
	CONNECTION_PROTOCOL,
	verifyCoreConnection,
	type ConnectionFailureReason,
} from './connection'
export { CoreInstanceStateManager, type CoreInstanceStateSnapshot } from './instance-state'
export {
	publishMessage,
	pendingMessages,
	writeReceipt,
	writeResult,
	waitForReceipt,
	waitForResult,
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
export { drainQueue } from './drain'
export * from './logic'
