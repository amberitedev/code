import { ConvexAmberiteAuthClient, type PlatformAdapter } from '@amberite/amberite-api'

import type { CookieOptions } from '#app'

export const AMBERITE_ACCESS_TOKEN_COOKIE = 'auth-token'
export const AMBERITE_REFRESH_TOKEN_COOKIE = 'amberite-refresh-token'

const amberiteCookieOptions = () =>
	({
		maxAge: 60 * 60 * 24 * 365 * 10,
		sameSite: 'lax',
		httpOnly: false,
		path: '/',
		secure: useRuntimeConfig().public.cookieSecure,
	}) satisfies CookieOptions<string | null>

export function useAmberiteAuthClient(): ConvexAmberiteAuthClient {
	return new ConvexAmberiteAuthClient({ adapter: createAmberiteWebAdapter() })
}

function createAmberiteWebAdapter(): PlatformAdapter {
	const config = useRuntimeConfig()
	const convexUrl = config.public.amberiteConvexUrl
	if (typeof convexUrl !== 'string' || convexUrl.trim() === '') {
		throw new Error('NUXT_PUBLIC_AMBERITE_CONVEX_URL must be configured for Amberite auth.')
	}

	const accessToken = useCookie<string | null>(
		AMBERITE_ACCESS_TOKEN_COOKIE,
		amberiteCookieOptions(),
	)
	const refreshToken = useCookie<string | null>(
		AMBERITE_REFRESH_TOKEN_COOKIE,
		amberiteCookieOptions(),
	)

	return {
		fetchFn: globalThis.fetch.bind(globalThis) as typeof fetch,
		convexUrl,
		getCoreUrl: async () => null,
		getCurrentJwt: async () => accessToken.value || null,
		setCurrentJwt: async (token) => {
			accessToken.value = token
		},
		getCurrentRefreshToken: async () => refreshToken.value || null,
		setCurrentRefreshToken: async (token) => {
			refreshToken.value = token
		},
		openExternalAuth: async (url) => {
			await navigateTo(url, { external: true })
		},
	}
}
