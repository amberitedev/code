import { defineEventHandler, getQuery, sendRedirect } from 'h3'

import {
	clearMinecraftAuthCookies,
	minecraftRedirectUri,
	normalizeMinecraftUuid,
	noStore,
	optionalMinecraftClientSecret,
	readMinecraftAuthFlow,
	requiredMinecraftClientId,
	signInWithMinecraftToken,
} from '~/server/utils/amberite-minecraft-auth'

interface MicrosoftTokenResponse {
	access_token?: string
}

interface XboxTokenResponse {
	Token?: string
	XErr?: number
	DisplayClaims?: { xui?: Array<{ uhs?: string }> }
}

interface MinecraftTokenResponse {
	access_token?: string
}

interface MinecraftProfileResponse {
	id?: string
	name?: string
}

export default defineEventHandler(async (event) => {
	noStore(event)
	let redirect = '/dashboard'
	try {
		const flow = await readMinecraftAuthFlow(event)
		redirect = flow.redirect
		const query = getQuery(event)
		const state = singleQueryValue(query.state)
		if (!state || state !== flow.state) throw new Error('minecraft_auth_state_invalid')
		if (singleQueryValue(query.error)) throw new Error('minecraft_auth_cancelled')
		const code = singleQueryValue(query.code)
		if (!code) throw new Error('minecraft_auth_state_invalid')

		const microsoftToken = await exchangeMicrosoftCode({
			clientId: requiredMinecraftClientId(event),
			clientSecret: optionalMinecraftClientSecret(event),
			code,
			redirectUri: minecraftRedirectUri(event),
			verifier: flow.verifier,
		})
		const xboxToken = await authenticateXboxLive(microsoftToken)
		const xstsToken = await authorizeXsts(xboxToken.token)
		const minecraftToken = await authenticateMinecraft(xstsToken.uhs, xstsToken.token)
		const profile = await requireMinecraftProfile(minecraftToken)
		if (flow.expectedMinecraftUuid && profile.id !== flow.expectedMinecraftUuid)
			throw new Error('minecraft_uuid_mismatch')
		await signInWithMinecraftToken(event, minecraftToken, flow.expectedMinecraftUuid)
		return sendRedirect(event, redirect, 302)
	} catch (error) {
		if (isServerConfigurationError(error)) throw error
		const code = publicErrorCode(error)
		console.error('Minecraft sign-in failed:', code)
		return sendRedirect(
			event,
			`/auth/sign-in?${new URLSearchParams({ error: code, redirect }).toString()}`,
			302,
		)
	} finally {
		clearMinecraftAuthCookies(event)
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
			headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
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
		'minecraft_xbox_restricted',
	)
	return requireXboxClaims(response)
}

async function authorizeXsts(userToken: string): Promise<{ token: string; uhs: string }> {
	const response = await fetchJson<XboxTokenResponse>(
		'https://xsts.auth.xboxlive.com/xsts/authorize',
		{
			method: 'POST',
			headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
			body: JSON.stringify({
				Properties: { SandboxId: 'RETAIL', UserTokens: [userToken] },
				RelyingParty: 'rp://api.minecraftservices.com/',
				TokenType: 'JWT',
			}),
		},
		'minecraft_xbox_restricted',
	)
	return requireXboxClaims(response)
}

function requireXboxClaims(response: XboxTokenResponse): { token: string; uhs: string } {
	const token = response.Token
	const uhs = response.DisplayClaims?.xui?.[0]?.uhs
	if (!token || !uhs) throw new Error('minecraft_xbox_restricted')
	return { token, uhs }
}

async function authenticateMinecraft(uhs: string, xstsToken: string): Promise<string> {
	const response = await fetchJson<MinecraftTokenResponse>(
		'https://api.minecraftservices.com/authentication/login_with_xbox',
		{
			method: 'POST',
			headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
			body: JSON.stringify({ identityToken: `XBL3.0 x=${uhs};${xstsToken}` }),
		},
		'minecraft_services_token_failed',
	)
	if (!response.access_token) throw new Error('minecraft_services_token_missing')
	return response.access_token
}

async function requireMinecraftProfile(
	minecraftAccessToken: string,
): Promise<{ id: string; name: string }> {
	const response = await fetch('https://api.minecraftservices.com/minecraft/profile', {
		headers: { Authorization: `Bearer ${minecraftAccessToken}`, Accept: 'application/json' },
	})
	if (response.status === 404) throw new Error('minecraft_java_profile_required')
	if (!response.ok) throw new Error('minecraft_profile_failed')
	const profile = (await response.json()) as MinecraftProfileResponse
	const id = normalizeMinecraftUuid(profile.id)
	if (!id || typeof profile.name !== 'string' || !profile.name) {
		throw new Error('minecraft_java_profile_required')
	}
	return { id, name: profile.name }
}

async function fetchJson<T>(url: string, init: RequestInit, errorCode: string): Promise<T> {
	let response: Response
	try {
		response = await fetch(url, init)
	} catch {
		throw new Error('minecraft_provider_unreachable')
	}
	if (response.status === 429) throw new Error('minecraft_provider_throttled')
	if (!response.ok) throw new Error(errorCode)
	return (await response.json()) as T
}

function singleQueryValue(value: unknown): string {
	return typeof value === 'string' ? value : ''
}

function publicErrorCode(error: unknown): string {
	const code = error instanceof Error ? error.message : 'minecraft_auth_failed'
	return code.startsWith('minecraft_') ? code : 'minecraft_auth_failed'
}

function isServerConfigurationError(error: unknown): boolean {
	return (
		typeof error === 'object' &&
		error !== null &&
		'statusCode' in error &&
		Number((error as { statusCode?: unknown }).statusCode) >= 500
	)
}
