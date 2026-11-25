import type { ContractClient } from '../contracts/client'
import type { AbstractModrinthClient } from '../core/abstract-client'
import type { AbstractModule } from '../core/abstract-module'

/** Methods of `M` a Core answers, with upstream's signatures. */
export type Adapter<M extends AbstractModule> = Partial<Omit<M, 'getModuleID'>>

/** The routes of the Core that has `serverId`. */
export async function coreOf(client: AbstractModrinthClient, serverId: string) {
	return client.core((await client.cores.coreOf(serverId)).id)
}

/** Run a long op on the Core that has `serverId`, waiting for it to end. */
export async function opOn(
	client: AbstractModrinthClient,
	serverId: string,
	start: (routes: ContractClient, op: string) => Promise<unknown>,
) {
	const { id } = await client.cores.coreOf(serverId)
	await client.cores.op(id, (op) => start(client.core(id), op))
}
