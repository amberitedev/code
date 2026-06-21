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
}
