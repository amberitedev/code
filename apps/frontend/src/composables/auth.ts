import type { AmberiteAccountUser } from '@amberite/amberite-api'
import type { Labrinth } from '@modrinth/api-client'
import { useStorage } from '@vueuse/core'
import type { LocationQueryValue, RouteLocationNormalizedLoaded } from 'vue-router'

import type { CookieOptions } from '#app'

import { useAmberiteAuthClient } from './amberite-client'

type AuthState = {
	user: (AmberiteAccountUser & Partial<Labrinth.Users.v2.User>) | null
	token: string
}

type QueryValue = LocationQueryValue | LocationQueryValue[] | undefined
type FullPathRoute = Pick<RouteLocationNormalizedLoaded, 'fullPath'>
type LauncherRoute = Pick<RouteLocationNormalizedLoaded, 'query'>

export const LAST_SIGN_IN_OAUTH_PROVIDER_STORAGE_KEY = 'auth-last-sign-in-oauth-provider'
export const PENDING_SIGN_IN_OAUTH_PROVIDER_STORAGE_KEY = 'auth-pending-sign-in-oauth-provider'

const AUTH_COOKIE_OPTIONS = {
	maxAge: 60 * 60 * 24 * 365 * 10,
	sameSite: 'lax',
	httpOnly: false,
	path: '/',
} satisfies CookieOptions<string | null>

const normalizeAuthToken = (value: unknown) => {
	if (typeof value === 'string') {
		return value
	}
	return ''
}

const getQueryString = (value: QueryValue) => {
	if (Array.isArray(value)) {
		return value[0] ?? null
	}
	return value ?? null
}

export const useAuth = async (oldToken: string | null | undefined = null) => {
	const auth = useState<AuthState>('auth', () => ({
		user: null,
		token: '',
	}))

	if (!auth.value.user || oldToken) {
		auth.value = await initAuth(oldToken)
	}

	return auth
}

export const initAuth = async (oldToken: string | null | undefined = null) => {
	const auth: AuthState = {
		user: null,
		token: '',
	}
	const authClient = useAmberiteAuthClient()

	if (oldToken === 'none') {
		await authClient.logOut()
		return auth
	}

	const route = useRoute()
	const config = useRuntimeConfig()
	const authCookie = useCookie<string | null>('auth-token', {
		...AUTH_COOKIE_OPTIONS,
		secure: config.public.cookieSecure,
	})

	if (oldToken) {
		const normalized = normalizeAuthToken(oldToken)
		if (normalized) {
			authCookie.value = normalized
		}
	}

	const oauthCode = normalizeAuthToken(route.query.code)
	if (oauthCode && !route.fullPath.includes('new_account=true')) {
		authCookie.value = oauthCode
	}

	if (route.fullPath.includes('new_account=true') && route.path !== '/auth/welcome') {
		const redirect = route.path.startsWith('/auth/') ? null : route.fullPath

		await navigateTo(
			`/auth/welcome?authToken=${oauthCode}${
				redirect ? `&redirect=${encodeURIComponent(redirect)}` : ''
			}`,
		)
	}

	const tokenStr = normalizeAuthToken(authCookie.value)

	if (authCookie.value != null && tokenStr === '') {
		authCookie.value = null
	} else if (tokenStr) {
		try {
			const session = tokenStr.startsWith('mra_')
				? await authClient.signInWithModrinthToken(tokenStr)
				: await authClient.restoreSession()
			if (session) {
				auth.token = session.tokens.token
				auth.user = session.user
				authCookie.value = session.tokens.token
			} else {
				authCookie.value = null
			}
		} catch {
			authCookie.value = null
			auth.token = ''
		}
	}

	return auth
}

export const getSignInRedirectPath = (route: FullPathRoute) => {
	const fullPath = route.fullPath
	if (fullPath === '/auth' || fullPath.startsWith('/auth/')) {
		return '/dashboard'
	}
	return fullPath
}

export const getSignInRouteObj = (route: FullPathRoute, redirectOverride?: string | null) => ({
	path: '/auth/sign-in',
	query: {
		redirect: redirectOverride ?? getSignInRedirectPath(route),
	},
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

				if (ipver) {
					callbackUrl.searchParams.set('ipver', ipver)
				}

				if (port) {
					callbackUrl.searchParams.set('port', port)
				}

				return callbackUrl.toString()
			})()
		: `${config.public.siteUrl}/auth/sign-in?redirect=${encodeURIComponent(redirect)}`

	return `${config.public.apiBaseUrl}auth/init?provider=${provider}&url=${encodeURIComponent(fullURL)}`
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

	const auth = await useAuth()

	await useBaseFetch('auth/provider', {
		method: 'DELETE',
		body: {
			provider,
		},
	})

	await useAuth(auth.value.token)

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
