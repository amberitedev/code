const trimTrailingSlash = (url: string) => url.replace(/\/$/, '')

const siteUrl = trimTrailingSlash(import.meta.env.MODRINTH_URL || 'https://modrinth.com')
const labrinthBaseUrl = trimTrailingSlash(
	import.meta.env.MODRINTH_API_BASE_URL || 'https://api.modrinth.com',
)
const archonBaseUrl = trimTrailingSlash(
	import.meta.env.MODRINTH_ARCHON_BASE_URL || 'https://archon.modrinth.com',
)
const sharedInstancesBaseUrl = trimTrailingSlash(
	import.meta.env.SHARED_INSTANCES_API_BASE_URL || 'https://shared-instances.modrinth.com',
)

export const config = {
	siteUrl,
	stripePublishableKey:
		import.meta.env.VITE_STRIPE_PUBLISHABLE_KEY ||
		'pk_test_51JbFxJJygY5LJFfKV50mnXzz3YLvBVe2Gd1jn7ljWAkaBlRz3VQdxN9mXcPSrFbSqxwAb0svte9yhnsmm7qHfcWn00R611Ce7b',
	labrinthBaseUrl,
	archonBaseUrl,
	sharedInstancesBaseUrl,
	convexUrl: import.meta.env.VITE_CONVEX_URL,
	convexSiteUrl: import.meta.env.VITE_CONVEX_SITE_URL,
	realtimeUrl: import.meta.env.VITE_REALTIME_URL,
}

export function applyDevAppConfig(devConfig: { convexUrl: string; convexSiteUrl: string }) {
	if (!import.meta.env.DEV) return
	config.convexUrl = devConfig.convexUrl
	config.convexSiteUrl = devConfig.convexSiteUrl
}
