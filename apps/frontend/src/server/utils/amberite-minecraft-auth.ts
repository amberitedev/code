import {
	AmberiteApiError,
	type AmberiteSession,
	type AmberiteSessionTokens,
	ConvexAmberiteAuthClient,
	type PlatformAdapter,
} from '@modrinth/api-client'
import {
	createError,
	deleteCookie,
	getCookie,
	getHeader,
	type H3Event,
	setCookie,
	setResponseHeader,
} from 'h3'

import { useRuntimeConfig } from '#imports'

import {
	createPkceChallenge,
	normalizeAuthRedirect,
	normalizeAuthUuid,
	randomAuthValue,
	requestIsSameOrigin,
	sealAuthFlow,
	type SealedMinecraftAuthFlow,
	unsealAuthFlow,
} from './amberite-auth-crypto'

export type MinecraftAuthIntent = 'continue' | 'use_another_account'
export type MinecraftAuthFlow = SealedMinecraftAuthFlow

export interface BrowserAmberiteSession {
	accessToken: string
}

export const MINECRAFT_AUTH_FLOW_COOKIE = 'amberite-minecraft-auth-flow'
export const AMBERITE_REFRESH_TOKEN_COOKIE = 'amberite-refresh-token'
export const LEGACY_ACCESS_TOKEN_COOKIE = 'auth-token'

const TEMP_COOKIE_MAX_AGE = 60 * 10
const REFRESH_COOKIE_MAX_AGE = 60 * 60 * 24 * 90
const REFRESH_COOKIE_PATH = '/api/amberite/session'

export function requiredMinecraftClientId(event: H3Event): string {
	const value = useRuntimeConfig(event).amberiteMinecraftOAuthClientId
	if (typeof value !== 'string' || value.trim() === '') {
		throw createError({
			statusCode: 500,
			message: 'AMBERITE_MINECRAFT_OAUTH_CLIENT_ID must be configured for Minecraft sign-in.',
		})
	}
	return value.trim()
}

export function optionalMinecraftClientSecret(event: H3Event): string | null {
	const value = useRuntimeConfig(event).amberiteMinecraftOAuthClientSecret
	return typeof value === 'string' && value.trim() ? value.trim() : null
}

export function minecraftRedirectUri(event: H3Event): string {
	return `${useRuntimeConfig(event).public.siteUrl}/api/amberite/minecraft/callback`
}

export function normalizeMinecraftAuthIntent(value: unknown): MinecraftAuthIntent {
	return value === 'use_another_account' ? 'use_another_account' : 'continue'
}

export function normalizeMinecraftUuid(value: unknown): string | undefined {
	try {
		return normalizeAuthUuid(value)
	} catch {
		throw createError({ statusCode: 400, message: 'Invalid UUID' })
	}
}

export const normalizeLocalRedirect = normalizeAuthRedirect

export function temporaryCookieOptions(event: H3Event) {
	return {
		httpOnly: true,
		maxAge: TEMP_COOKIE_MAX_AGE,
		path: '/',
		sameSite: 'lax' as const,
		secure: Boolean(useRuntimeConfig(event).public.cookieSecure),
	}
}

export async function setMinecraftAuthFlow(event: H3Event, flow: MinecraftAuthFlow) {
	setCookie(
		event,
		MINECRAFT_AUTH_FLOW_COOKIE,
		await sealAuthFlow(flow, requiredFlowSecret(event)),
		temporaryCookieOptions(event),
	)
}

export async function readMinecraftAuthFlow(event: H3Event): Promise<MinecraftAuthFlow> {
	const value = getCookie(event, MINECRAFT_AUTH_FLOW_COOKIE)
	if (!value) throw new Error('minecraft_auth_state_invalid')
	return await unsealAuthFlow(value, requiredFlowSecret(event))
}

export function clearMinecraftAuthCookies(event: H3Event): void {
	deleteCookie(event, MINECRAFT_AUTH_FLOW_COOKIE, { path: '/' })
}

export function clearLegacyBrowserAuthCookies(event: H3Event): void {
	deleteCookie(event, LEGACY_ACCESS_TOKEN_COOKIE, { path: '/' })
}

export const randomBase64Url = randomAuthValue
export const pkceChallenge = createPkceChallenge

export async function signInWithMinecraftToken(
	event: H3Event,
	minecraftAccessToken: string,
	expectedMinecraftUuid?: string,
): Promise<AmberiteSession> {
	return await createServerAuthClient(event).signInWithMinecraftToken({
		minecraftAccessToken,
		expectedMinecraftUuid,
	})
}

export async function restoreBrowserSession(
	event: H3Event,
): Promise<BrowserAmberiteSession | null> {
	assertSameOrigin(event)
	noStore(event)
	clearLegacyBrowserAuthCookies(event)
	const refreshToken = getCookie(event, AMBERITE_REFRESH_TOKEN_COOKIE)
	if (!refreshToken) return null
	try {
		const session = await createServerAuthClient(event, { token: '', refreshToken }).refreshSession(
			refreshToken,
		)
		return session ? { accessToken: session.tokens.token } : null
	} catch (error) {
		if (error instanceof AmberiteApiError && error.recovery === 'clear_session') {
			clearBrowserSessionCookies(event)
			throw createError({ statusCode: 401, message: error.message })
		}
		throw createError({
			statusCode: 503,
			message: error instanceof Error ? error.message : 'Amberite session restore failed',
		})
	}
}

export async function signOutBrowserSession(event: H3Event): Promise<void> {
	assertSameOrigin(event)
	noStore(event)
	const refreshToken = getCookie(event, AMBERITE_REFRESH_TOKEN_COOKIE)
	try {
		if (refreshToken) {
			const client = createServerAuthClient(event, { token: '', refreshToken })
			await client.refreshSession(refreshToken)
			await client.logOut()
		}
	} finally {
		clearBrowserSessionCookies(event)
	}
}

export function clearBrowserSessionCookies(event: H3Event): void {
	deleteCookie(event, LEGACY_ACCESS_TOKEN_COOKIE, { path: '/' })
	deleteCookie(event, AMBERITE_REFRESH_TOKEN_COOKIE, { path: '/' })
	deleteCookie(event, AMBERITE_REFRESH_TOKEN_COOKIE, { path: REFRESH_COOKIE_PATH })
}

export function assertSameOrigin(event: H3Event): void {
	const sameOrigin = requestIsSameOrigin({
		expectedOrigin: new URL(useRuntimeConfig(event).public.siteUrl).origin,
		origin: getHeader(event, 'origin'),
		referer: getHeader(event, 'referer'),
		fetchSite: getHeader(event, 'sec-fetch-site'),
	})
	if (!sameOrigin)
		throw createError({ statusCode: 403, message: 'Session request origin rejected' })
}

export function noStore(event: H3Event): void {
	setResponseHeader(event, 'Cache-Control', 'no-store, max-age=0')
	setResponseHeader(event, 'Pragma', 'no-cache')
}

function createServerAuthClient(
	event: H3Event,
	initialTokens: AmberiteSessionTokens | null = null,
): ConvexAmberiteAuthClient {
	const convexUrl = useRuntimeConfig(event).public.amberiteConvexUrl
	if (typeof convexUrl !== 'string' || convexUrl.trim() === '') {
		throw createError({
			statusCode: 500,
			message: 'NUXT_PUBLIC_AMBERITE_CONVEX_URL must be configured for Amberite auth.',
		})
	}
	let tokens = initialTokens
	const adapter: PlatformAdapter = {
		fetchFn: globalThis.fetch.bind(globalThis) as typeof fetch,
		convexUrl,
		getCurrentJwt: async () => tokens?.token || null,
		setCurrentJwt: async (token) => {
			tokens = tokens ? { ...tokens, token: token ?? '' } : null
		},
		amberiteSessionStorage: {
			read: async () => tokens,
			write: async (next) => {
				tokens = next
				setRefreshCookie(event, next.refreshToken)
			},
			clear: async () => {
				tokens = null
				clearBrowserSessionCookies(event)
			},
		},
	}
	return new ConvexAmberiteAuthClient({ adapter })
}

function setRefreshCookie(event: H3Event, value: string): void {
	deleteCookie(event, AMBERITE_REFRESH_TOKEN_COOKIE, { path: '/' })
	setCookie(event, AMBERITE_REFRESH_TOKEN_COOKIE, value, {
		httpOnly: true,
		maxAge: REFRESH_COOKIE_MAX_AGE,
		path: REFRESH_COOKIE_PATH,
		sameSite: 'lax',
		secure: Boolean(useRuntimeConfig(event).public.cookieSecure),
	})
	deleteCookie(event, LEGACY_ACCESS_TOKEN_COOKIE, { path: '/' })
}

function requiredFlowSecret(event: H3Event): string {
	const secret = useRuntimeConfig(event).amberiteAuthCookieSecret
	if (typeof secret !== 'string' || secret.length < 32) {
		throw createError({
			statusCode: 500,
			message: 'AMBERITE_AUTH_COOKIE_SECRET must contain at least 32 characters.',
		})
	}
	return secret
}
