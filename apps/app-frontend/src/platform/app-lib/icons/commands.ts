import { library } from '@modrinth/api-client'

import { apiClient } from '@/platform/modrinth-client'

/** Writes the icon bytes to the app's icon cache and returns the cached file path. */
export function cacheIconBytes(iconBytes: number[]): Promise<string> {
	return apiClient.local.call(library.cacheIcon, { body: new Blob([new Uint8Array(iconBytes)]) })
}
