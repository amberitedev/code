import type { PlatformAdapter } from '@amberite/api-lib'
import { createClient } from '@supabase/supabase-js'
import { invoke } from '@tauri-apps/api/core'
import { fetch as tauriFetch } from '@tauri-apps/plugin-http'
import { openUrl } from '@tauri-apps/plugin-opener'

let cachedAdapter: PlatformAdapter | null = null

/**
 * Desktop PlatformAdapter for Amberite.
 *
 * - fetchFn routes through Tauri's HTTP plugin to bypass WebView CORS.
 * - openExternalAuth opens the system browser via Tauri's opener plugin.
 * - getCurrentJwt reads from the active Supabase session (V1).
 * - getCoreToken and getCoreUrl read from Rust via Tauri invoke.
 */
export function createDesktopAdapter(): PlatformAdapter {
	const supabaseUrl = import.meta.env.VITE_SUPABASE_URL as string
	const supabaseAnonKey = import.meta.env.VITE_SUPABASE_ANON_KEY as string
	if (!supabaseUrl || !supabaseAnonKey) {
		throw new Error('Supabase URL or anon key not configured')
	}

	const supabase = createClient(supabaseUrl, supabaseAnonKey, {
		auth: {
			autoRefreshToken: true,
			persistSession: true,
			storageKey: 'amberite-auth',
		},
		global: {
			fetch: tauriFetch as unknown as typeof fetch,
		},
	})

	return {
		fetchFn: tauriFetch as unknown as typeof fetch,

		openExternalAuth: async (url: string) => {
			await openUrl(url)
		},

		getCurrentJwt: async () => {
			const { data, error } = await supabase.auth.getSession()
			if (error || !data.session) return null
			return data.session.access_token
		},

		getLocalCoreToken: async () => {
			try {
				return await invoke<string>('plugin:amberite|get_local_core_token')
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

		supabase,
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
