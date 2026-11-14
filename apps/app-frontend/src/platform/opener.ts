import { ContractClient, desktop } from '@modrinth/api-client'

import { localTransport } from '@/platform/local'

const local = new ContractClient(localTransport)

/** Keeps `@tauri-apps/plugin-opener`'s name. The link opens on the App's desktop. */
export async function openUrl(url: string | URL): Promise<void> {
	await local.call(desktop.openUrl, { body: { url: String(url) } })
}
