import type { AbstractModrinthClient } from '../core/abstract-client'
import type { KyrosBackupsV1Module } from '../modules/kyros/backups/v1'
import type { Adapter } from './adapter'
import { link, linkUrl, serverPath } from './links'

const download = (nodeUrlHost: string, worldId: string, backupId: string) =>
	serverPath(
		nodeUrlHost,
		`/v1/worlds/${encodeURIComponent(worldId)}/backups/${encodeURIComponent(backupId)}/download`,
	)

export function kyrosBackups(client: AbstractModrinthClient): Adapter<KyrosBackupsV1Module> {
	return {
		authorizeDownload: (nodeUrlHost, worldId, backupId) =>
			link(client, nodeUrlHost, download(nodeUrlHost, worldId, backupId)),
		getDownloadUrl: (nodeUrlHost, worldId, backupId, token) =>
			linkUrl(nodeUrlHost, download(nodeUrlHost, worldId, backupId), token),
	}
}
