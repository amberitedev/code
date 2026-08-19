import type { AmberiteSessionTokens } from './session'

export interface PlatformMinecraftSignInRequest {
	mode: 'continue' | 'use_another_account'
	expectedMinecraftUuid?: string
}

export interface PlatformAuthSession {
	accessToken: string
	expiresAt?: string
	/** Verified current-user projection returned by native coordinators when already available. */
	user?: unknown
}

export interface PlatformAmberiteSessionStorage {
	read(): Promise<AmberiteSessionTokens | null>
	write(tokens: AmberiteSessionTokens): Promise<void>
	clear(): Promise<void>
}

/** Platform-owned capabilities used by Amberite clients. */
export interface PlatformAdapter {
	/** HTTP fetch function. On desktop this is tauriFetch; on web it is native fetch. */
	fetchFn: typeof fetch

	/** Convex deployment URL, e.g. https://...convex.cloud. */
	convexUrl: string
	/** Convex HTTP Actions origin, e.g. https://...convex.site. */
	convexSiteUrl?: string

	/** Return the direct Core HTTP URL, e.g. "http://localhost:16662". */
	getCoreUrl(): Promise<string | null>

	/** Return the identity the current Core URL is linked to, when the platform has one. */
	getConnectedCoreId?(): Promise<string | null>

	/** Return the current short-lived Amberite access JWT. */
	getCurrentJwt(): Promise<string | null>
	setCurrentJwt?(jwt: string | null): Promise<void>
	setCurrentAmberiteUserId?(userId: string | null): Promise<void>

	/** Legacy split token access. New platforms should implement atomic session methods. */
	getCurrentRefreshToken?(): Promise<string | null>
	setCurrentRefreshToken?(refreshToken: string | null): Promise<void>

	/** Atomically read, replace, and clear a complete product session. */
	amberiteSessionStorage?: PlatformAmberiteSessionStorage

	/** Refresh the platform-owned product session without exposing its refresh token. */
	refreshAmberiteSession?(): Promise<PlatformAuthSession | null>

	/** Native Minecraft sign-in. Desktop implements this entirely outside the WebView. */
	signInWithMinecraft?(request: PlatformMinecraftSignInRequest): Promise<PlatformAuthSession>

	/** Native product-session restoration and sign-out. */
	restoreAmberiteSession?(): Promise<PlatformAuthSession | null>
	signOutAmberiteSession?(): Promise<void>

	/** Return the one-time local setup secret for an app-launched Core, if present. */
	getLocalSetupSecret?(): Promise<string | null>

	/** Open an external auth window/redirect. Desktop opens system browser; web redirects page. */
	openExternalAuth(url: string): void | Promise<void>
}
