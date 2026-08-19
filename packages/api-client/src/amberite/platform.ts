export interface PlatformMinecraftSignInRequest {
	mode: 'continue' | 'use_another_account'
	expectedMinecraftUuid?: string
}

export interface PlatformAuthSession {
	accessToken: string
	expiresAt?: string
	user?: unknown
}

export interface AmberiteSessionTokens {
	token: string
	refreshToken: string
}

export interface PlatformAmberiteSessionStorage {
	read(): Promise<AmberiteSessionTokens | null>
	write(tokens: AmberiteSessionTokens): Promise<void>
	clear(): Promise<void>
}

/** Platform-owned capabilities used by Amberite identity and social requests. */
export interface AmberitePlatformAdapter {
	fetchFn: typeof fetch
	convexUrl: string
	convexSiteUrl?: string
	getCurrentJwt(): Promise<string | null>
	setCurrentJwt?(jwt: string | null): Promise<void>
	setCurrentAmberiteUserId?(userId: string | null): Promise<void>
	getCurrentRefreshToken?(): Promise<string | null>
	setCurrentRefreshToken?(refreshToken: string | null): Promise<void>
	amberiteSessionStorage?: PlatformAmberiteSessionStorage
	refreshAmberiteSession?(): Promise<PlatformAuthSession | null>
	signInWithMinecraft?(request: PlatformMinecraftSignInRequest): Promise<PlatformAuthSession>
	restoreAmberiteSession?(): Promise<PlatformAuthSession | null>
	signOutAmberiteSession?(): Promise<void>
}
