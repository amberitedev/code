import type { AbstractModrinthClient } from '../core/abstract-client'
import { actions } from './actions'
import { backups } from './backups'
import { backupsQueue } from './backups-queue'
import { content } from './content'
import { filesV0, filesV1 } from './files'
import { icons } from './icons'
import { kyrosBackups } from './kyros-backups'
import { kyrosContent } from './kyros-content'
import { kyrosLogs } from './logs'
import { options } from './options'
import { properties } from './properties'
import { serverUsers } from './server-users'
import { serversV0 } from './servers-v0'
import { serversV1 } from './servers-v1'
import { uploadSessions } from './upload-sessions'

/**
 * With self-hosted hosting every server is on a Core: the upstream archon and kyros modules'
 * methods are answered by adapters over `client.core(id)`, in upstream's shapes, so `packages/ui`
 * stays upstream's.
 */
export function withCoreHosting(client: AbstractModrinthClient): void {
	const { archon } = client
	Object.assign(archon.servers_v0, serversV0(client))
	Object.assign(archon.servers_v1, serversV1(client))
	Object.assign(archon.backups_queue_v1, backupsQueue(client))
	Object.assign(archon.backups_v1, backups(client))
	Object.assign(archon.options_v1, options(client))
	Object.assign(archon.properties_v1, properties(client))
	Object.assign(archon.server_users_v1, serverUsers(client))
	Object.assign(archon.actions_v1, actions(client))
	Object.assign(archon.content_v1, content(client))
	Object.assign(archon.icons_v1, icons(client))
	const { kyros } = client
	Object.assign(kyros.backups_v1, kyrosBackups(client))
	Object.assign(kyros.files_v0, filesV0(client))
	Object.assign(kyros.files_v1, filesV1(client))
	Object.assign(kyros.upload_sessions_v1, uploadSessions(client))
	Object.assign(kyros.logs_v1, kyrosLogs())
	Object.assign(kyros.content_v1, kyrosContent(client))
}
