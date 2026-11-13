import type { AbstractModrinthClient } from '../core/abstract-client'
import type { AbstractModule } from '../core/abstract-module'

/** Methods of `M` a Core answers, with upstream's signatures. */
export type Adapter<M extends AbstractModule> = Partial<Omit<M, 'getModuleID'>>

/** The routes of the Core that has `serverId`. */
export async function coreOf(client: AbstractModrinthClient, serverId: string) {
	return client.core((await client.archon.cores_v1.coreOf(serverId)).id)
}
