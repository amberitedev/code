import type { AbstractModrinthClient } from '../core/abstract-client'
import { actions } from './actions'
import { backups } from './backups'
import { backupsQueue } from './backups-queue'
import { options } from './options'
import { properties } from './properties'
import { serverUsers } from './server-users'
import { serversV0 } from './servers-v0'
import { serversV1 } from './servers-v1'

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
}
