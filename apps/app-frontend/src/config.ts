const trimTrailingSlash = (url: string) => url.replace(/\/$/, '')

const siteUrl = trimTrailingSlash(import.meta.env.MODRINTH_URL || 'https://modrinth.com')
const labrinthBaseUrl = trimTrailingSlash(
	import.meta.env.MODRINTH_API_BASE_URL || 'https://api.modrinth.com',
)

export const config = {
	siteUrl,
	accountApiUrl: trimTrailingSlash(import.meta.env.VITE_ACCOUNT_API_URL || ''),
	accountWebUrl: trimTrailingSlash(import.meta.env.VITE_ACCOUNT_WEB_URL || ''),
	labrinthBaseUrl,
	/**
	 * Amberite's account site, where share and server invite links and profile links open. A getter,
	 * because the dev App sets `accountApiUrl` after start.
	 */
	get accountSiteUrl() {
		return this.accountWebUrl || this.accountApiUrl
	},
}

export function applyDevAppConfig(devConfig: { backendUrl?: string }) {
	if (!import.meta.env.DEV) return
	if (devConfig.backendUrl) config.accountApiUrl = trimTrailingSlash(devConfig.backendUrl)
}
