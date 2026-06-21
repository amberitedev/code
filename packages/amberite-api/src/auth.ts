import type { PlatformAdapter } from './adapter'
import { AuthError } from './errors'

const MICROSOFT_AUTH_URL = 'https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize'

export interface MicrosoftLoginRequest {
	state: string
	codeVerifier: string
}

export interface AuthSession {
	userId: string
	displayName: string
	email?: string
	accessToken: string
}

export async function startMicrosoftLogin(
	adapter: PlatformAdapter,
	redirectUri: string,
): Promise<MicrosoftLoginRequest> {
	const clientId = getMicrosoftClientId()
	const state = generateState()
	const codeVerifier = generateCodeVerifier()
	const params = new URLSearchParams({
		client_id: clientId,
		response_type: 'code',
		redirect_uri: redirectUri,
		response_mode: 'query',
		scope: 'openid profile email XboxLive.signin',
		state,
		code_challenge: await sha256Base64Url(codeVerifier),
		code_challenge_method: 'S256',
	})
	await adapter.openExternalAuth(`${MICROSOFT_AUTH_URL}?${params.toString()}`)
	return { state, codeVerifier }
}

export async function completeMicrosoftLogin(
	adapter: PlatformAdapter,
	code: string,
	redirectUri: string,
	codeVerifier?: string,
): Promise<AuthSession> {
	const endpoint = getAuthExchangeUrl()
	const res = await adapter.fetchFn(endpoint, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({
			code,
			redirectUri,
			...(codeVerifier ? { codeVerifier } : {}),
		}),
	})

	if (!res.ok) {
		const text = await res.text().catch(() => 'unknown')
		throw new AuthError(`auth exchange failed: ${res.status} ${text}`)
	}

	const session = (await res.json()) as AuthSession
	if (!session.accessToken || !session.userId) throw new AuthError('invalid auth exchange response')
	await adapter.setCurrentJwt?.(session.accessToken)
	return session
}

function getMicrosoftClientId(): string {
	const id = env('VITE_MICROSOFT_CLIENT_ID', 'MICROSOFT_CLIENT_ID')
	if (!id) throw new AuthError('Microsoft client ID not configured')
	return id
}

function getAuthExchangeUrl(): string {
	const url = env('VITE_AUTH_EXCHANGE_URL', 'AUTH_EXCHANGE_URL')
	if (!url) throw new AuthError('auth exchange URL not configured')
	return url
}

function env(importMetaKey: string, processKey: string): string {
	if (typeof import.meta !== 'undefined' && (import.meta as any).env?.[importMetaKey]) {
		return (import.meta as any).env[importMetaKey]
	}
	if (typeof process !== 'undefined') return process.env[processKey] ?? ''
	return ''
}

function generateState(): string {
	return randomBase64Url(16)
}

function generateCodeVerifier(): string {
	return randomBase64Url(32)
}

function randomBase64Url(length: number): string {
	const bytes = new Uint8Array(length)
	if (typeof crypto !== 'undefined' && crypto.getRandomValues) crypto.getRandomValues(bytes)
	else for (let i = 0; i < bytes.length; i++) bytes[i] = Math.floor(Math.random() * 256)
	return base64Url(bytes)
}

async function sha256Base64Url(value: string): Promise<string> {
	if (typeof crypto === 'undefined' || !crypto.subtle) {
		throw new AuthError('WebCrypto is required for Microsoft PKCE')
	}
	const data = new TextEncoder().encode(value)
	return base64Url(new Uint8Array(await crypto.subtle.digest('SHA-256', data)))
}

function base64Url(bytes: Uint8Array): string {
	let binary = ''
	for (const byte of bytes) binary += String.fromCharCode(byte)
	if (typeof btoa === 'undefined') throw new AuthError('base64 encoder is not available')
	const base64 = btoa(binary)
	return base64.replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}
