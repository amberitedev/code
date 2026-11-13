import type { AbstractModrinthClient } from '../core/abstract-client'
import { ModrinthApiError } from '../core/errors'
import type { KyrosFilesV1Module } from '../modules/kyros/files/v1'
import type { Adapter } from './adapter'
import { link, linkUrl } from './links'

const world = (worldId: string) => `/v1/worlds/${encodeURIComponent(worldId)}`

export function filesV1(client: AbstractModrinthClient): Adapter<KyrosFilesV1Module> {
	// `getFileDownloadUrl` is not given the file, so each token keeps the path it was made for.
	const files = new Map<string, string>()
	return {
		async authorizeFileDownload(nodeUrlHost, worldId, path) {
			const file = `${world(worldId)}/files/contents-raw?path=${encodeURIComponent(path)}`
			const made = await link(client, nodeUrlHost, file)
			files.set(made.token, file)
			return made
		},
		getFileDownloadUrl(nodeUrlHost, _worldId, token) {
			const file = files.get(token)
			if (!file) throw new ModrinthApiError('This download link was used', { statusCode: 401 })
			files.delete(token)
			return linkUrl(nodeUrlHost, file, token)
		},
		authorizeFullWorldDownload: (nodeUrlHost, worldId) =>
			link(client, nodeUrlHost, `${world(worldId)}/files/download-full-zip`),
		getFullWorldDownloadUrl: (nodeUrlHost, worldId, token) =>
			linkUrl(nodeUrlHost, `${world(worldId)}/files/download-full-zip`, token),
	}
}
