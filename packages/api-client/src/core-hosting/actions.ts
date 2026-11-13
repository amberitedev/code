import { core } from '@amberite/contracts'

import type { AbstractModrinthClient } from '../core/abstract-client'
import { ModrinthApiError } from '../core/errors'
import type { ArchonActionsV1Module } from '../modules/archon/actions/v1'
import { actionLog, onlyForOwner } from './access'
import { type Adapter, coreOf } from './adapter'

/** A Core's activity log, the owner's only. Its members name the users it mentions. */
export function actions(client: AbstractModrinthClient): Adapter<ArchonActionsV1Module> {
	return {
		async list(serverId, options = {}) {
			const access = await coreOf(client, serverId)
			const [page, members] = await Promise.all([
				access
					.call(core.ListActivity, {
						query: {
							server_id: serverId,
							limit: options.limit,
							before: options.offset || undefined,
						},
					})
					.catch(onlyForOwner),
				access.call(core.ListMembers, { params: { id: serverId } }).catch(onlyForOwner),
			])
			return actionLog(page ?? { entries: [], next_before: null }, members, options)
		},
		async clear() {
			throw new ModrinthApiError('A Core keeps its activity log', { statusCode: 400 })
		},
	}
}
