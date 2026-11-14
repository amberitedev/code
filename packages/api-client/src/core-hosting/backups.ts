import { core } from '@amberite/contracts'

import type { AbstractModrinthClient } from '../core/abstract-client'
import type { ArchonBackupsV1Module } from '../modules/archon/backups/v1'
import { type Adapter, coreOf } from './adapter'

/** Only renaming is left of the old backups API; the queue (`backupsQueue`) does the rest. */
export function backups(client: AbstractModrinthClient): Adapter<ArchonBackupsV1Module> {
	return {
		async rename(serverId, _worldId, backupId, request) {
			if (request.name === undefined) return
			await (
				await coreOf(client, serverId)
			).call(core.RenameBackup, {
				params: { id: serverId, backup_id: backupId },
				body: { name: request.name },
			})
		},
	}
}
