const trimTrailingSlash = (url: string) => url.replace(/\/$/, '')

const siteUrl = trimTrailingSlash(import.meta.env.MODRINTH_URL)
const labrinthBaseUrl = trimTrailingSlash(import.meta.env.MODRINTH_API_BASE_URL)
const archonBaseUrl = trimTrailingSlash(import.meta.env.MODRINTH_ARCHON_BASE_URL)

export const config = {
	siteUrl,
	stripePublishableKey: import.meta.env.VITE_STRIPE_PUBLISHABLE_KEY,
	labrinthBaseUrl,
	archonBaseUrl,
	convexUrl: import.meta.env.VITE_CONVEX_URL,
	convexSiteUrl: import.meta.env.VITE_CONVEX_SITE_URL,
	/** Optional rollout flag. When unset, the desktop keeps durable Convex state only. */
	realtimeUrl: import.meta.env.VITE_REALTIME_URL,
}
