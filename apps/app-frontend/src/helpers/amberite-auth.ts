import { invoke } from '@tauri-apps/api/core'

import { getDesktopAdapter } from '@/adapters/desktop'

export async function ensureAmberiteSession(options: { interactive?: boolean } = {}) {
	const adapter = getDesktopAdapter()
	const token = await invoke<string | null>('plugin:amberite|convex_refresh_session', {
		convexUrl: adapter.convexUrl,
	})
	if (token) return token
	if (!options.interactive) return null
	return await invoke<string>('plugin:amberite|convex_login', {
		convexUrl: adapter.convexUrl,
	})
}

export async function signOutAmberite() {
	await invoke('plugin:amberite|convex_logout')
}
