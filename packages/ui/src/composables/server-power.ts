import type { PowerAction } from '@amberite/contracts/core'
import type { AbstractModrinthClient } from '@modrinth/api-client'
import { core as coreRoutes } from '@amberite/contracts/core'

/** Starts, stops, restarts or kills a server: a long op on its Core, which ends once done. */
export function changeServerPower(
	client: AbstractModrinthClient,
	coreId: string,
	serverId: string,
	action: PowerAction,
) {
	return client.cores.op(coreId, (op) =>
		client.core(coreId).call(coreRoutes.ChangePower, {
			params: { id: serverId },
			body: { action, op },
		}),
	)
}
