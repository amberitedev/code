import { core } from '@amberite/contracts'

import type { AbstractModrinthClient } from '../core/abstract-client'
import type { ArchonOptionsV1Module } from '../modules/archon/options/v1'
import { type Adapter, coreOf } from './adapter'
import { sameCommand } from './servers-v0'

/** The command line is reported, not edited; what changes is the managed Java major. */
export function options(client: AbstractModrinthClient): Adapter<ArchonOptionsV1Module> {
	return {
		async getStartup(serverId) {
			const setup = await (
				await coreOf(client, serverId)
			).call(core.GetSetup, {
				params: { id: serverId },
			})
			return {
				java_version: setup.java_version,
				jre_vendor: null,
				original_invocation: setup.invocation,
				startup_command: setup.invocation,
			}
		},
		async patchStartup(serverId, _worldId, body) {
			const routes = await coreOf(client, serverId)
			const params = { id: serverId }
			const { invocation } = await routes.call(core.GetSetup, { params })
			if (invocation !== null) sameCommand(body.startup_command, invocation)
			await routes.call(core.SetJavaVersion, {
				params,
				body: { major: body.java_version ?? null },
			})
		},
	}
}
