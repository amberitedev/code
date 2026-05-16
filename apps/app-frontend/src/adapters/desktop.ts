import type { PlatformAdapter } from '@amberite/api-lib'
import { invoke } from '@tauri-apps/api/core'
import { fetch as tauriFetch } from '@tauri-apps/plugin-http'
import { openUrl } from '@tauri-apps/plugin-opener'

let cachedAdapter: PlatformAdapter | null = null

/**
 * Desktop PlatformAdapter for Amberite.
 *
 * - fetchFn routes through Tauri's HTTP plugin to bypass WebView CORS.
 * - openExternalAuth opens the system browser via Tauri's opener plugin.
 * - getCurrentJwt and setCurrentJwt use the OS keychain via Tauri invoke.
 * - getLocalSetupSecret reads the one-time setup secret for app-launched Cores.
 * - getCoreUrl reads from Rust via Tauri invoke.
 */
export function createDesktopAdapter(): PlatformAdapter {
	const convexUrl = import.meta.env.VITE_CONVEX_URL as string
	if (!convexUrl) {
		throw new Error('Convex URL not configured')
	}

	return {
		fetchFn: tauriFetch as unknown as typeof fetch,
		convexUrl,

		openExternalAuth: async (url: string) => {
			await openUrl(url)
		},

		getCurrentJwt: async () => {
			try {
				return await invoke<string | null>('plugin:amberite|get_current_jwt')
			} catch {
				return null
			}
		},

		setCurrentJwt: async (jwt: string | null) => {
			if (jwt) await invoke('plugin:amberite|set_current_jwt', { jwt })
			else await invoke('plugin:amberite|clear_current_jwt')
		},

		getLocalSetupSecret: async () => {
			try {
				return await invoke<string | null>('plugin:amberite|get_local_setup_secret')
			} catch {
				return null
			}
		},

		getCoreUrl: async () => {
			try {
				return await invoke<string>('plugin:amberite|core_get_url')
			} catch {
				return null
			}
		},
	}
}

/**
 * Return a singleton desktop adapter, creating it on first call.
 */
export function getDesktopAdapter(): PlatformAdapter {
	if (!cachedAdapter) {
		cachedAdapter = createDesktopAdapter()
	}
	return cachedAdapter
}
