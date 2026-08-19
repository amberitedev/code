import type {
	AmberiteAccountUser,
	AmberiteApiError,
	RecoveryDisposition,
} from '@modrinth/api-client'
import { useStorage } from '@vueuse/core'
import type { LocationQueryValue, RouteLocationNormalizedLoaded } from 'vue-router'

import { clearAmberiteAccessToken, useAmberiteAuthClient } from './amberite-client'

export type AmberiteAuthGate =
	| 'restoring'
	| 'signedOut'
	| 'verifying'
	| 'retryableOffline'
	| 'authenticated'

export interface RememberedAmberiteIdentity {
	version: 1
	minecraftUuid: string
	avatarUrl: string | null
	displayName: string
	verifiedMinecraftHandle: string
	lastSuccessfulSignIn: string
}

type AuthState = {
	user: AmberiteAccountUser | null
	token: string
	status: AmberiteAuthGate
	error: string | null
	recovery: RecoveryDisposition | null
}

type QueryValue = LocationQueryValue | LocationQueryValue[] | undefined
type FullPathRoute = Pick<RouteLocationNormalizedLoaded, 'fullPath'>
type LauncherRoute = Pick<RouteLocationNormalizedLoaded, 'query'>

export const LAST_SIGN_IN_OAUTH_PROVIDER_STORAGE_KEY = 'auth-last-sign-in-oauth-provider'
export const PENDING_SIGN_IN_OAUTH_PROVIDER_STORAGE_KEY = 'auth-pending-sign-in-oauth-provider'
export const REMEMBERED_AMBERITE_IDENTITY_KEY = 'amberite-remembered-identity-v1'
export type AmberiteMinecraftAuthIntent = 'continue' | 'use_another_account'

let initialization: Promise<AuthState> | null = null

const getQueryString = (value: QueryValue) => {
	if (Array.isArray(value)) return value[0] ?? null
	return value ?? null
}

export const useAuth = async (oldToken: string | null | undefined = null) => {
	const auth = useState<AuthState>('auth', () => emptyAuth('restoring'))
	if (oldToken === 'none') {
		await useAmberiteAuthClient().logOut()
		clearAmberiteAccessToken()
		initialization = null
		auth.value = emptyAuth('signedOut')
		return auth
	}
	if (!initialization || auth.value.status === 'retryableOffline') {
		initialization = initAuth()
		auth.value = await initialization
	}
	return auth
}

export const initAuth = async (): Promise<AuthState> => {
	if (!import.meta.client) return emptyAuth('restoring')
	try {
		const session = await useAmberiteAuthClient().restoreSession()
		if (!session) return emptyAuth('signedOut')
		rememberIdentity(session.user)
		return {
			user: session.user,
			// Amberite JWTs stay inside the Amberite adapter and must never reach Labrinth callers.
			token: '',
			status: 'authenticated',
			error: null,
			recovery: null,
		}
	} catch (error) {
		const recovery = recoveryOf(error)
		if (recovery === 'preserve_and_retry') {
			return {
				...emptyAuth('retryableOffline'),
				error: error instanceof Error ? error.message : String(error),
				recovery,
			}
		}
		clearAmberiteAccessToken()
		return {
			...emptyAuth('signedOut'),
			error: error instanceof Error ? error.message : String(error),
			recovery,
		}
	}
}

export async function retryAuthRestore() {
	const auth = useState<AuthState>('auth')
	if (auth.value.status !== 'restoring' || !initialization) {
		auth.value = emptyAuth('restoring')
		initialization = initAuth()
	}
	auth.value = await initialization
	return auth
}

export function setAuthVerifying() {
	const auth = useState<AuthState>('auth')
	auth.value = { ...auth.value, status: 'verifying', error: null }
}

export function getRememberedAmberiteIdentity(): RememberedAmberiteIdentity | null {
	if (!import.meta.client) return null
	try {
		const value: unknown = JSON.parse(
			localStorage.getItem(REMEMBERED_AMBERITE_IDENTITY_KEY) ?? 'null',
		)
		return isRememberedIdentity(value) ? value : null
	} catch {
		return null
	}
}

export const getSignInRedirectPath = (route: FullPathRoute) => {
	const fullPath = route.fullPath
	return fullPath === '/auth' || fullPath.startsWith('/auth/') ? '/dashboard' : fullPath
}

export const getSignInRouteObj = (route: FullPathRoute, redirectOverride?: string | null) => ({
	path: '/auth/sign-in',
	query: { redirect: redirectOverride ?? getSignInRedirectPath(route) },
})

export const getAuthUrl = (provider: string, redirect = '/dashboard') => {
	const config = useRuntimeConfig()
	const route = useNativeRoute()
	const launcher = getQueryString(route.query.launcher)
	const fullURL = launcher
		? (() => {
				const callbackUrl = new URL('/auth/sign-in', config.public.siteUrl)
				callbackUrl.searchParams.set('launcher', launcher)
				const ipver = getQueryString(route.query.ipver)
				const port = getQueryString(route.query.port)
				if (ipver) callbackUrl.searchParams.set('ipver', ipver)
				if (port) callbackUrl.searchParams.set('port', port)
				return callbackUrl.toString()
			})()
		: `${config.public.siteUrl}/auth/sign-in?redirect=${encodeURIComponent(redirect)}`
	return `${config.public.apiBaseUrl}auth/init?provider=${provider}&url=${encodeURIComponent(fullURL)}`
}

export const getMinecraftAuthUrl = (
	intent: AmberiteMinecraftAuthIntent = 'continue',
	redirect = '/dashboard',
) => {
	const config = useRuntimeConfig()
	const url = new URL('/api/amberite/minecraft/start', config.public.siteUrl)
	url.searchParams.set('intent', intent)
	url.searchParams.set('redirect', normalizeRedirect(redirect))
	if (intent === 'continue') {
		const remembered = getRememberedAmberiteIdentity()
		if (remembered) url.searchParams.set('expectedMinecraftUuid', remembered.minecraftUuid)
	}
	return `${url.pathname}${url.search}`
}

export const promotePendingSignInOAuthProvider = () => {
	if (!import.meta.client) return
	const pending = useStorage<string | null>(
		PENDING_SIGN_IN_OAUTH_PROVIDER_STORAGE_KEY,
		null,
		undefined,
		{ initOnMounted: true },
	)
	if (!pending.value) return
	const last = useStorage<string | null>(LAST_SIGN_IN_OAUTH_PROVIDER_STORAGE_KEY, null, undefined, {
		initOnMounted: true,
	})
	last.value = pending.value
	pending.value = null
}

export const removeAuthProvider = async (provider: string) => {
	startLoading()
	await useBaseFetch('auth/provider', { method: 'DELETE', body: { provider } })
	await retryAuthRestore()
	stopLoading()
}

export const getLauncherRedirectUrl = (route: LauncherRoute) => {
	const ipver = getQueryString(route.query.ipver)
	const port = Number(getQueryString(route.query.port))
	const usesLocalhostRedirectionScheme = ['4', '6'].includes(ipver ?? '') && port < 65536
	return usesLocalhostRedirectionScheme
		? `http://${ipver === '4' ? '127.0.0.1' : '[::1]'}:${port}`
		: 'https://launcher-files.modrinth.com'
}

function emptyAuth(status: AmberiteAuthGate): AuthState {
	return { user: null, token: '', status, error: null, recovery: null }
}

function isRememberedIdentity(value: unknown): value is RememberedAmberiteIdentity {
	if (!value || typeof value !== 'object') return false
	const identity = value as Record<string, unknown>
	return (
		identity.version === 1 &&
		typeof identity.minecraftUuid === 'string' &&
		/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
			identity.minecraftUuid,
		) &&
		(identity.avatarUrl === null || typeof identity.avatarUrl === 'string') &&
		typeof identity.displayName === 'string' &&
		typeof identity.verifiedMinecraftHandle === 'string' &&
		typeof identity.lastSuccessfulSignIn === 'string'
	)
}

function rememberIdentity(user: AmberiteAccountUser): void {
	if (!import.meta.client) return
	const value: RememberedAmberiteIdentity = {
		version: 1,
		minecraftUuid: user.minecraftUuid,
		avatarUrl: user.avatar_url,
		displayName: user.name,
		verifiedMinecraftHandle: user.verifiedMinecraftHandle,
		lastSuccessfulSignIn: new Date().toISOString(),
	}
	try {
		localStorage.setItem(REMEMBERED_AMBERITE_IDENTITY_KEY, JSON.stringify(value))
	} catch {
		// Remembering the last identity is optional and must not break sign-in.
	}
}

function recoveryOf(error: unknown): RecoveryDisposition {
	return typeof error === 'object' && error !== null && 'recovery' in error
		? ((error as AmberiteApiError).recovery ?? 'clear_session')
		: 'preserve_and_retry'
}

function normalizeRedirect(value: string): string {
	try {
		const base = new URL('https://amberite.local')
		const redirect = new URL(value, base)
		return redirect.origin === base.origin
			? `${redirect.pathname}${redirect.search}${redirect.hash}`
			: '/dashboard'
	} catch {
		return '/dashboard'
	}
}
