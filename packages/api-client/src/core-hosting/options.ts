import { core } from '@amberite/contracts'

import type { AbstractModrinthClient } from '../core/abstract-client'
import type { ArchonOptionsV1Module } from '../modules/archon/options/v1'
import { type Adapter, coreOf } from './adapter'
import { sameCommand } from './servers-v0'

/** The command line is reported, not edited; what changes is the managed Java major. */
export function options(client: AbstractModrinthClient): Adapter<ArchonOptionsV1Module> {
	return {
		async getStartup(serverId) {
			const startup = await (
				await coreOf(client, serverId)
			).call(core.GetStartup, {
				params: { id: serverId },
			})
			return { ...startup, jre_vendor: null, startup_command: startup.original_invocation }
		},
		async patchStartup(serverId, _worldId, body) {
			const routes = await coreOf(client, serverId)
			const params = { id: serverId }
			const { original_invocation } = await routes.call(core.GetStartup, { params })
			if (original_invocation !== null) sameCommand(body.startup_command, original_invocation)
			await routes.call(core.ChangeStartup, {
				params,
				body: { java_version: body.java_version ?? null },
			})
		},
	}
}
