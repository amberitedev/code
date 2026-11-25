import { core } from '@amberite/contracts'

import type { AbstractModrinthClient } from '../core/abstract-client'
import type { ArchonPropertiesV1Module } from '../modules/archon/properties/v1'
import { type Adapter, coreOf } from './adapter'

export function properties(client: AbstractModrinthClient): Adapter<ArchonPropertiesV1Module> {
	return {
		async getProperties(serverId) {
			return (await coreOf(client, serverId)).call(core.GetProperties, {
				params: { id: serverId },
			})
		},
		async patchProperties(serverId, _worldId, body) {
			return (await coreOf(client, serverId)).call(core.ChangeProperties, {
				params: { id: serverId },
				body,
			})
		},
	}
}
