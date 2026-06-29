import {
	ConvexAmberiteAuthClient,
	type PlatformAdapter,
	type AmberiteSession,
} from '@amberite/amberite-api'
import { useRuntimeConfig } from '#imports'
import { createError, deleteCookie, getCookie, setCookie, type H3Event } from 'h3'

export type MinecraftAuthMode = 'signin' | 'signup'

export const MINECRAFT_AUTH_STATE_COOKIE = 'amberite-minecraft-auth-state'
export const MINECRAFT_AUTH_VERIFIER_COOKIE = 'amberite-minecraft-auth-verifier'
export const MINECRAFT_AUTH_MODE_COOKIE = 'amberite-minecraft-auth-mode'
export const MINECRAFT_AUTH_REDIRECT_COOKIE = 'amberite-minecraft-auth-redirect'

const ACCESS_TOKEN_COOKIE = 'auth-token'
const REFRESH_TOKEN_COOKIE = 'amberite-refresh-token'
const TEMP_COOKIE_MAX_AGE = 60 * 10
const SESSION_COOKIE_MAX_AGE = 60 * 60 * 24 * 365 * 10

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

export function normalizeMinecraftAuthMode(value: unknown): MinecraftAuthMode {
	return value === 'signup' ? 'signup' : 'signin'
}

export function normalizeLocalRedirect(value: unknown): string {
	if (typeof value !== 'string') return '/dashboard'
	if (!value.startsWith('/') || value.startsWith('//')) return '/dashboard'
	return value
}

export function temporaryCookieOptions(event: H3Event) {
	return {
		httpOnly: true,
		maxAge: TEMP_COOKIE_MAX_AGE,
		path: '/',
		sameSite: 'lax' as const,
		secure: Boolean(useRuntimeConfig(event).public.cookieSecure),
	}
}

export function clearMinecraftAuthCookies(event: H3Event): void {
	for (const cookie of [
		MINECRAFT_AUTH_STATE_COOKIE,
		MINECRAFT_AUTH_VERIFIER_COOKIE,
		MINECRAFT_AUTH_MODE_COOKIE,
		MINECRAFT_AUTH_REDIRECT_COOKIE,
	]) {
		deleteCookie(event, cookie, { path: '/' })
	}
}

export function randomBase64Url(byteLength: number): string {
	const bytes = crypto.getRandomValues(new Uint8Array(byteLength))
	return base64Url(bytes)
}

export async function pkceChallenge(verifier: string): Promise<string> {
	const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(verifier))
	return base64Url(new Uint8Array(digest))
}

export async function signInWithMinecraftToken(
	event: H3Event,
	minecraftAccessToken: string,
): Promise<AmberiteSession> {
	const convexUrl = useRuntimeConfig(event).public.amberiteConvexUrl
	if (typeof convexUrl !== 'string' || convexUrl.trim() === '') {
		throw createError({
			statusCode: 500,
			message: 'NUXT_PUBLIC_AMBERITE_CONVEX_URL must be configured for Amberite auth.',
		})
	}

	let accessToken = getCookie(event, ACCESS_TOKEN_COOKIE) ?? null
	let refreshToken = getCookie(event, REFRESH_TOKEN_COOKIE) ?? null

	const adapter: PlatformAdapter = {
		fetchFn: globalThis.fetch.bind(globalThis) as typeof fetch,
		convexUrl,
		getCoreUrl: async () => null,
		getCurrentJwt: async () => accessToken,
		setCurrentJwt: async (token) => {
			accessToken = token
			setSessionCookie(event, ACCESS_TOKEN_COOKIE, token)
		},
		getCurrentRefreshToken: async () => refreshToken,
		setCurrentRefreshToken: async (token) => {
			refreshToken = token
			setSessionCookie(event, REFRESH_TOKEN_COOKIE, token)
		},
		openExternalAuth: async () => {},
	}

	return await new ConvexAmberiteAuthClient({ adapter }).signInWithMinecraftToken({
		minecraftAccessToken,
	})
}

function setSessionCookie(event: H3Event, name: string, value: string | null): void {
	if (!value) {
		deleteCookie(event, name, { path: '/' })
		return
	}

	setCookie(event, name, value, {
		httpOnly: false,
		maxAge: SESSION_COOKIE_MAX_AGE,
		path: '/',
		sameSite: 'lax',
		secure: Boolean(useRuntimeConfig(event).public.cookieSecure),
	})
}

function base64Url(bytes: Uint8Array): string {
	let value = ''
	for (const byte of bytes) value += String.fromCharCode(byte)
	return btoa(value).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '')
}
