/**
 * Auth module — Microsoft login flow shared across desktop and web.
 *
 * 1. Build Microsoft OAuth URL and open it via the adapter.
 * 2. Receive Microsoft token from callback.
 * 3. Exchange via the `microsoft-auth` Supabase Edge Function.
 * 4. Call supabase.auth.setSession() to authenticate the SDK.
 * 5. Adapter persists the JWT (desktop: OS keychain; web: cookie handled by SDK).
 */

import type { PlatformAdapter } from './adapter'
import { AuthError, NetworkError } from './errors'

const MICROSOFT_CLIENT_ID = 'amberite-microsoft-client-id' // injected by build / env
const MICROSOFT_AUTH_URL = 'https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize'
const MICROSOFT_TOKEN_URL = 'https://login.microsoftonline.com/consumers/oauth2/v2.0/token'

export interface MicrosoftTokens {
	accessToken: string
	idToken: string
}

export interface AuthSession {
	userId: string
	displayName: string
	email?: string
}

/**
 * Build the Microsoft OAuth URL and tell the adapter to open it.
 */
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
	const url = `${MICROSOFT_AUTH_URL}?${params.toString()}`
	await adapter.openExternalAuth(url)
}

/**
 * Exchange a Microsoft authorization code for tokens, then call the
 * `microsoft-auth` Edge Function to create/look up a Supabase user.
 */
export async function completeMicrosoftLogin(
	adapter: PlatformAdapter,
	code: string,
	redirectUri: string,
): Promise<AuthSession> {
	const clientId = getMicrosoftClientId()
	const clientSecret = getMicrosoftClientSecret()

	// 1. Exchange code for Microsoft tokens
	const tokenRes = await adapter.fetchFn(MICROSOFT_TOKEN_URL, {
		method: 'POST',
		headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
		body: new URLSearchParams({
			client_id: clientId,
			client_secret: clientSecret,
			code,
			redirect_uri: redirectUri,
			grant_type: 'authorization_code',
		}),
	})

	if (!tokenRes.ok) {
		throw new AuthError(`Microsoft token exchange failed: ${tokenRes.status}`)
	}

	const tokenData = await tokenRes.json()
	const accessToken = tokenData.access_token as string
	const idToken = tokenData.id_token as string
	if (!accessToken) throw new AuthError('No access_token in Microsoft response')

	// 2. Call the microsoft-auth Edge Function
	const supabaseUrl = adapter.supabase.supabaseUrl
	const functionUrl = `${supabaseUrl}/functions/v1/microsoft-auth`

	const functionRes = await adapter.fetchFn(functionUrl, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ access_token: accessToken, id_token: idToken }),
	})

	if (!functionRes.ok) {
		const text = await functionRes.text().catch(() => 'unknown')
		throw new AuthError(`microsoft-auth Edge Function failed: ${functionRes.status} ${text}`)
	}

	const authData = await functionRes.json()
	const { access_token, refresh_token, expires_in, user_id, display_name } = authData
	if (!access_token || !user_id) {
		throw new AuthError('Invalid response from microsoft-auth Edge Function')
	}

	// 3. Set Supabase session
	const { error } = await adapter.supabase.auth.setSession({
		access_token,
		refresh_token: refresh_token ?? '',
	})
	if (error) throw new AuthError(`setSession failed: ${error.message}`)

	return { userId: user_id, displayName: display_name }
}

function getMicrosoftClientId(): string {
	const id =
		typeof import.meta !== 'undefined' && (import.meta as any).env?.VITE_MICROSOFT_CLIENT_ID
			? (import.meta as any).env.VITE_MICROSOFT_CLIENT_ID
			: typeof process !== 'undefined'
				? process.env.MICROSOFT_CLIENT_ID
				: ''
	if (!id) throw new AuthError('Microsoft client ID not configured')
	return id
}

function getMicrosoftClientSecret(): string {
	const secret =
		typeof import.meta !== 'undefined' && (import.meta as any).env?.VITE_MICROSOFT_CLIENT_SECRET
			? (import.meta as any).env.VITE_MICROSOFT_CLIENT_SECRET
			: typeof process !== 'undefined'
				? process.env.MICROSOFT_CLIENT_SECRET
				: ''
	if (!secret) throw new AuthError('Microsoft client secret not configured')
	return secret
}

function generateState(): string {
	const arr = new Uint8Array(16)
	if (typeof crypto !== 'undefined' && crypto.getRandomValues) {
		crypto.getRandomValues(arr)
	} else {
		for (let i = 0; i < arr.length; i++) arr[i] = Math.floor(Math.random() * 256)
	}
	return Array.from(arr, (b) => b.toString(16).padStart(2, '0')).join('')
}
