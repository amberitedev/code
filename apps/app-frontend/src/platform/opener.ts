import { desktop } from '@modrinth/api-client'

import { apiClient } from '@/platform/modrinth-client'

/** Keeps `@tauri-apps/plugin-opener`'s name. The link opens on the App's desktop. */
export async function openUrl(url: string | URL): Promise<void> {
	await apiClient.local.call(desktop.openUrl, { body: { url: String(url) } })
}
