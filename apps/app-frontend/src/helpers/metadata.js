import { metadata } from '@modrinth/api-client'

import { apiClient } from '@/platform/modrinth-client'

// Gets the given loader versions from minecraft-meta
// Returns Manifest
// Untyped, as it was over invoke: callers read it with their own Manifest type.
/** @returns {Promise<any>} */
export async function get_loader_versions(loader) {
	return await apiClient.local.call(metadata.GetLoaderManifest, { params: { loader } })
}
