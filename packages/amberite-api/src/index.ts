export type {
	AmberiteUser,
	AmberiteProfile,
	AmberitePublicProfile,
	ProfileView,
	ProfileViewUserSummary,
	ProfileRelationship,
	ProfileSectionVisibility,
	ProfileVisibilitySettings,
	LinkedModrinthAccount,
	CoreListEntry,
	CorePresence,
	FriendGroupInfo,
	FriendGroupPublicProfile,
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

export { ConvexApiClient } from './convex-api'
export { AmberiteModrinthCompatFeature } from './modrinth-compat-feature'
export type { AmberiteModrinthCompatFeatureOptions } from './modrinth-compat-feature'
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
	AmberiteDevAccountSignInRequest,
	AmberiteMinecraftSignInRequest,
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
export { mapAmberiteUserToAccountUser, normalizeAmberiteAccountUser } from './profile'
export type { AmberiteAccountUser, AmberiteProfilePatch } from './profile'
export { composeSocialSessionState } from './social-session'
export type {
	DurableSocialSessionState,
	LiveSocialState,
	LiveUserState,
	SocialSessionState,
} from './social-session'
export { RealtimePresenceSession } from './realtime'
export type { RealtimeFrame, RealtimePresenceSessionOptions, RealtimeSocket } from './realtime'
export type {
	PlatformAdapter,
	PlatformAuthSession,
	PlatformMinecraftSignInRequest,
} from './adapter'
export {
	AmberiteApiError,
	NetworkError,
	AuthError,
	ProviderAuthError,
	ConvexError,
	authErrorFromNative,
	authErrorFromPayload,
	parseAuthFailurePayload,
} from './errors'
export type {
	AuthFailureCode,
	AuthFailurePayload,
	NativeAuthOperation,
	RecoveryDisposition,
} from './errors'
export * from './logic'
