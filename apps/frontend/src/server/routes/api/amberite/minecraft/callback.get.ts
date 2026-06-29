import { defineEventHandler, getCookie, getQuery, sendRedirect } from 'h3'

import {
	MINECRAFT_AUTH_MODE_COOKIE,
	MINECRAFT_AUTH_REDIRECT_COOKIE,
	MINECRAFT_AUTH_STATE_COOKIE,
	MINECRAFT_AUTH_VERIFIER_COOKIE,
	clearMinecraftAuthCookies,
	minecraftRedirectUri,
	normalizeLocalRedirect,
	normalizeMinecraftAuthMode,
	optionalMinecraftClientSecret,
	requiredMinecraftClientId,
	signInWithMinecraftToken,
} from '~/server/utils/amberite-minecraft-auth'

interface MicrosoftTokenResponse {
	access_token?: string
}

interface XboxTokenResponse {
	Token?: string
	DisplayClaims?: {
		xui?: Array<{
			uhs?: string
		}>
	}
}

interface MinecraftTokenResponse {
	access_token?: string
}

interface MinecraftProfileResponse {
	id?: string
	name?: string
}

export default defineEventHandler(async (event) => {
	const clientId = requiredMinecraftClientId(event)
	const clientSecret = optionalMinecraftClientSecret(event)
	const redirectUri = minecraftRedirectUri(event)
	const mode = normalizeMinecraftAuthMode(getCookie(event, MINECRAFT_AUTH_MODE_COOKIE))
	const redirect = normalizeLocalRedirect(getCookie(event, MINECRAFT_AUTH_REDIRECT_COOKIE))

	try {
		const query = getQuery(event)
		if (typeof query.error === 'string') throw new Error('minecraft_auth_cancelled')

		const state = singleQueryValue(query.state)
		const code = singleQueryValue(query.code)
		const expectedState = getCookie(event, MINECRAFT_AUTH_STATE_COOKIE)
		const verifier = getCookie(event, MINECRAFT_AUTH_VERIFIER_COOKIE)
		if (!state || !expectedState || state !== expectedState || !code || !verifier) {
			throw new Error('minecraft_auth_state_invalid')
		}

		const microsoftToken = await exchangeMicrosoftCode({
			clientId,
			clientSecret,
			code,
			redirectUri,
			verifier,
		})
		const xboxToken = await authenticateXboxLive(microsoftToken)
		const xstsToken = await authorizeXsts(xboxToken.token)
		const minecraftToken = await authenticateMinecraft(xstsToken.uhs, xstsToken.token)
		await requireMinecraftProfile(minecraftToken)
		await signInWithMinecraftToken(event, minecraftToken)
		clearMinecraftAuthCookies(event)

		const target =
			mode === 'signup'
				? `/auth/almost-there?redirect=${encodeURIComponent(redirect)}`
				: redirect
		return sendRedirect(event, target, 302)
	} catch (error) {
		if (isServerConfigurationError(error)) throw error
		console.error('Minecraft sign-in failed:', error)
		clearMinecraftAuthCookies(event)
		return sendRedirect(event, errorRedirectPath(mode, redirect), 302)
	}
})

async function exchangeMicrosoftCode(args: {
	clientId: string
	clientSecret: string | null
	code: string
	redirectUri: string
	verifier: string
}): Promise<string> {
	const body = new URLSearchParams({
		client_id: args.clientId,
		code: args.code,
		code_verifier: args.verifier,
		grant_type: 'authorization_code',
		redirect_uri: args.redirectUri,
	})
	if (args.clientSecret) body.set('client_secret', args.clientSecret)

	const response = await fetchJson<MicrosoftTokenResponse>(
		'https://login.microsoftonline.com/consumers/oauth2/v2.0/token',
		{
			method: 'POST',
			headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
			body,
		},
		'minecraft_microsoft_token_failed',
	)

	if (!response.access_token) throw new Error('minecraft_microsoft_token_missing')
	return response.access_token
}

async function authenticateXboxLive(accessToken: string): Promise<{ token: string; uhs: string }> {
	const response = await fetchJson<XboxTokenResponse>(
		'https://user.auth.xboxlive.com/user/authenticate',
		{
			method: 'POST',
			headers: {
				'Content-Type': 'application/json',
				Accept: 'application/json',
			},
			body: JSON.stringify({
				Properties: {
					AuthMethod: 'RPS',
					SiteName: 'user.auth.xboxlive.com',
					RpsTicket: `d=${accessToken}`,
				},
				RelyingParty: 'http://auth.xboxlive.com',
				TokenType: 'JWT',
			}),
		},
		'minecraft_xbox_token_failed',
	)

	const token = response.Token
	const uhs = response.DisplayClaims?.xui?.[0]?.uhs
	if (!token || !uhs) throw new Error('minecraft_xbox_token_missing')
	return { token, uhs }
}

async function authorizeXsts(userToken: string): Promise<{ token: string; uhs: string }> {
	const response = await fetchJson<XboxTokenResponse>(
		'https://xsts.auth.xboxlive.com/xsts/authorize',
		{
			method: 'POST',
			headers: {
				'Content-Type': 'application/json',
				Accept: 'application/json',
			},
			body: JSON.stringify({
				Properties: {
					SandboxId: 'RETAIL',
					UserTokens: [userToken],
				},
				RelyingParty: 'rp://api.minecraftservices.com/',
				TokenType: 'JWT',
			}),
		},
		'minecraft_xsts_token_failed',
	)

	const token = response.Token
	const uhs = response.DisplayClaims?.xui?.[0]?.uhs
	if (!token || !uhs) throw new Error('minecraft_xsts_token_missing')
	return { token, uhs }
}

async function authenticateMinecraft(uhs: string, xstsToken: string): Promise<string> {
	const response = await fetchJson<MinecraftTokenResponse>(
		'https://api.minecraftservices.com/authentication/login_with_xbox',
		{
			method: 'POST',
			headers: {
				'Content-Type': 'application/json',
				Accept: 'application/json',
			},
			body: JSON.stringify({
				identityToken: `XBL3.0 x=${uhs};${xstsToken}`,
			}),
		},
		'minecraft_services_token_failed',
	)

	if (!response.access_token) throw new Error('minecraft_services_token_missing')
	return response.access_token
}

async function requireMinecraftProfile(minecraftAccessToken: string): Promise<void> {
	const response = await fetchJson<MinecraftProfileResponse>(
		'https://api.minecraftservices.com/minecraft/profile',
		{
			headers: {
				Authorization: `Bearer ${minecraftAccessToken}`,
				Accept: 'application/json',
			},
		},
		'minecraft_profile_required',
	)
	if (!response.id || !response.name) throw new Error('minecraft_profile_required')
}

async function fetchJson<T>(url: string, init: RequestInit, errorCode: string): Promise<T> {
	const response = await fetch(url, init)
	if (!response.ok) throw new Error(errorCode)
	return (await response.json()) as T
}

function singleQueryValue(value: unknown): string {
	if (Array.isArray(value)) return typeof value[0] === 'string' ? value[0] : ''
	return typeof value === 'string' ? value : ''
}

function errorRedirectPath(mode: 'signin' | 'signup', redirect: string): string {
	const path = mode === 'signup' ? '/auth/sign-up' : '/auth/sign-in'
	const query = new URLSearchParams({ error: 'minecraft_auth_failed' })
	if (redirect !== '/dashboard') query.set('redirect', redirect)
	return `${path}?${query.toString()}`
}

function isServerConfigurationError(error: unknown): boolean {
	return (
		typeof error === 'object' &&
		error !== null &&
		'statusCode' in error &&
		Number((error as { statusCode?: unknown }).statusCode) >= 500
	)
}
