import type { AbstractModrinthClient } from '../core/abstract-client'
import type { KyrosBackupsV1Module } from '../modules/kyros/backups/v1'
import type { Adapter } from './adapter'
import { authorize, coreUrl, serverPath } from './links'

const download = (nodeUrlHost: string, backupId: string) =>
	serverPath(nodeUrlHost, `/backups/${encodeURIComponent(backupId)}/download`)

export function kyrosBackups(client: AbstractModrinthClient): Adapter<KyrosBackupsV1Module> {
	return {
		authorizeDownload: (nodeUrlHost) => authorize(client, nodeUrlHost),
		getDownloadUrl: (nodeUrlHost, _worldId, backupId) =>
			coreUrl(nodeUrlHost, download(nodeUrlHost, backupId)),
	}
}
