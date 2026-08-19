export { AmberiteApiClient, type AmberiteProfile } from './client'
export { AmberiteApiClient as ConvexApiClient } from './client'
export {
	AmberiteAuthClient,
	type AmberiteAuthClientOptions,
	type AmberiteDevAccountSignInRequest,
	type AmberiteMinecraftTokenSignInRequest,
	type AmberiteMinecraftSignInRequest,
	type AmberiteSession,
} from './auth-client'
export { AmberiteAuthClient as ConvexAmberiteAuthClient } from './auth-client'
export {
	AmberiteApiError,
	AmberiteAuthError,
	AmberiteConvexError,
	AmberiteNetworkError,
	AmberiteProviderAuthError,
	authErrorFromNative,
	authErrorFromPayload,
	authErrorFromResponse,
	parseAuthFailurePayload,
	type AuthFailureCode,
	type AuthFailurePayload,
	type NativeAuthOperation,
	type RecoveryDisposition,
} from './errors'
export {
	AmberiteAuthError as AuthError,
	AmberiteConvexError as ConvexError,
	AmberiteNetworkError as NetworkError,
	AmberiteProviderAuthError as ProviderAuthError,
} from './errors'
export type {
	AmberitePlatformAdapter,
	AmberiteSessionTokens,
	PlatformAmberiteSessionStorage,
	PlatformAuthSession,
	PlatformMinecraftSignInRequest,
} from './platform'
export type { AmberitePlatformAdapter as PlatformAdapter } from './platform'
export {
	mapAmberiteUserToAccountUser,
	normalizeAmberiteAccountUser,
	type AmberiteAccountUser,
	type AmberiteProfilePatch,
} from './profile'
export {
	adapterSessionStorage,
	createMemoryAmberiteSessionStorage,
	isAmberiteSessionTokens,
	refreshPlatformAmberiteSession,
	validateAmberiteSessionTokens,
	type AmberiteSessionStorage,
} from './session'
export * from './logic'
export {
	RealtimePresenceSession,
	type RealtimeFrame,
	type RealtimePresenceSessionOptions,
	type RealtimeSocket,
} from './realtime'
export { ConvexAmberiteTransport, type AmberiteTransport } from './transport'
