import type { SupabaseClient } from '@supabase/supabase-js'

/**
 * PlatformAdapter abstracts the only two differences between desktop (Tauri) and web:
 * 1. fetch — desktop uses @tauri-apps/plugin-http (tauriFetch); web uses native fetch.
 * 2. Token storage — desktop uses OS keychain; web uses Supabase HTTP-only cookies.
 *
 * Everything else (auth logic, Core endpoints, relay transport) is identical.
 */
export interface PlatformAdapter {
	/** HTTP fetch function. On desktop this is tauriFetch; on web it is native fetch. */
	fetchFn: typeof fetch

	/** Pre-built Supabase client configured for the current platform. */
	supabase: SupabaseClient

	/** Return the local Core bearer token, if available (desktop only). */
	getLocalCoreToken(): Promise<string | null>

	/** Return the direct Core HTTP URL, e.g. "http://localhost:16662". */
	getCoreUrl(): Promise<string | null>

	/** Return the current user's Supabase JWT, if authenticated. */
	getCurrentJwt(): Promise<string | null>

	/** Open an external auth window/redirect. Desktop opens system browser; web redirects page. */
	openExternalAuth(url: string): void | Promise<void>
}
