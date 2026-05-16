import type { PlatformAdapter } from './adapter'
import { AuthError } from './errors'

const MICROSOFT_AUTH_URL = 'https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize'

export interface AuthSession {
	userId: string
	displayName: string
	email?: string
	accessToken: string
}

export async function startMicrosoftLogin(
	adapter: PlatformAdapter,
	redirectUri: string,
): Promise<void> {
	const clientId = getMicrosoftClientId()
	const params = new URLSearchParams({
		client_id: clientId,
		response_type: 'code',
		redirect_uri: redirectUri,
		response_mode: 'query',
		scope: 'openid profile email XboxLive.signin',
		state: generateState(),
	})
	await adapter.openExternalAuth(`${MICROSOFT_AUTH_URL}?${params.toString()}`)
}

export async function completeMicrosoftLogin(
	adapter: PlatformAdapter,
	code: string,
	redirectUri: string,
): Promise<AuthSession> {
	const endpoint = getAuthExchangeUrl()
	const res = await adapter.fetchFn(endpoint, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ code, redirectUri }),
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
	const arr = new Uint8Array(16)
	if (typeof crypto !== 'undefined' && crypto.getRandomValues) crypto.getRandomValues(arr)
	else for (let i = 0; i < arr.length; i++) arr[i] = Math.floor(Math.random() * 256)
	return Array.from(arr, (b) => b.toString(16).padStart(2, '0')).join('')
}
