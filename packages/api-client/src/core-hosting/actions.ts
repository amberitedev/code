import { core } from '@amberite/contracts'

import type { AbstractModrinthClient } from '../core/abstract-client'
import { ModrinthApiError } from '../core/errors'
import type { ArchonActionsV1Module } from '../modules/archon/actions/v1'
import { actionLog } from './access'
import { type Adapter, coreOf } from './adapter'

/** A Core's activity log. */
export function actions(client: AbstractModrinthClient): Adapter<ArchonActionsV1Module> {
	return {
		async list(serverId, options = {}) {
			const page = await (
				await coreOf(client, serverId)
			).call(core.ListActivity, {
				query: { server_id: serverId, limit: options.limit, before: options.offset || undefined },
			})
			return actionLog(page, options)
		},
		async clear() {
			throw new ModrinthApiError('A Core keeps its activity log', { statusCode: 400 })
		},
	}
}
