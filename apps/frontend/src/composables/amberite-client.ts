import {
	AuthError,
	ConvexAmberiteAuthClient,
	ConvexApiClient,
	NetworkError,
	type PlatformAdapter,
	type PlatformAuthSession,
} from '@amberite/amberite-api'

let browserAccessToken: string | null = null
let refreshPromise: Promise<PlatformAuthSession | null> | null = null
let sessionGeneration = 0

export function useAmberiteAuthClient(): ConvexAmberiteAuthClient {
	return new ConvexAmberiteAuthClient({ adapter: createAmberiteWebAdapter() })
}

export function useAmberiteSocialClient(): ConvexApiClient {
	return new ConvexApiClient(createAmberiteWebAdapter())
}

export function clearAmberiteAccessToken(): void {
	sessionGeneration += 1
	browserAccessToken = null
}

function createAmberiteWebAdapter(): PlatformAdapter {
	const config = useRuntimeConfig()
	const convexUrl = config.public.amberiteConvexUrl
	if (typeof convexUrl !== 'string' || convexUrl.trim() === '') {
		throw new Error('NUXT_PUBLIC_AMBERITE_CONVEX_URL must be configured for Amberite auth.')
	}

	return {
		fetchFn: globalThis.fetch.bind(globalThis) as typeof fetch,
		convexUrl,
		getCoreUrl: async () => null,
		getCurrentJwt: async () => (import.meta.client ? browserAccessToken : null),
		setCurrentJwt: async (token) => {
			if (import.meta.client) browserAccessToken = token
		},
		restoreMinecraftSession: async () => await requestServerSession('restore'),
		refreshAmberiteSession: async () => await refreshServerSession(),
		signOutMinecraftSession: async () => {
			clearAmberiteAccessToken()
			try {
				await $fetch('/api/amberite/session/logout', {
					method: 'POST',
					credentials: 'same-origin',
				})
			} finally {
				browserAccessToken = null
			}
		},
		openExternalAuth: async (url) => {
			await navigateTo(url, { external: true })
		},
	}
}

async function refreshServerSession(): Promise<PlatformAuthSession | null> {
	if (refreshPromise) return await refreshPromise
	refreshPromise = requestServerSession('refresh').finally(() => {
		refreshPromise = null
	})
	return await refreshPromise
}

async function requestServerSession(
	action: 'restore' | 'refresh',
): Promise<PlatformAuthSession | null> {
	if (!import.meta.client) return null
	const generation = sessionGeneration
	try {
		const response = await $fetch<{ accessToken: string } | null>(
			`/api/amberite/session/${action}`,
			{
				method: 'POST',
				credentials: 'same-origin',
			},
		)
		if (generation !== sessionGeneration) return null
		browserAccessToken = response?.accessToken ?? null
		return response ? { accessToken: response.accessToken } : null
	} catch (error) {
		if (generation !== sessionGeneration) return null
		const status = Number(
			(error as { statusCode?: unknown; status?: unknown }).statusCode ??
				(error as { status?: unknown }).status,
		)
		if (status === 401) {
			browserAccessToken = null
			throw new AuthError('session is invalid or expired', 'invalid_session')
		}
		throw new NetworkError('Amberite session service is unreachable', 'amberite_unreachable')
	}
}
