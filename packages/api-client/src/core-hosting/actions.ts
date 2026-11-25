import { core } from '@amberite/contracts'

import type { AbstractModrinthClient } from '../core/abstract-client'
import { ModrinthApiError } from '../core/errors'
import type { ArchonActionsV1Module } from '../modules/archon/actions/v1'
import type { Archon } from '../modules/archon/types'
import { type Adapter, coreOf } from './adapter'

/** Shown as the actor of what the Core's local CLI did. */
const LOCAL_CLI = 'core-cli'

/** A Core's activity log, filtered by the Core. */
export function actions(client: AbstractModrinthClient): Adapter<ArchonActionsV1Module> {
	return {
		async list(serverId, options = {}) {
			const { filter, ...rest } = options
			const page = await (
				await coreOf(client, serverId)
			).call(core.ListActivity, {
				query: {
					...rest,
					server_id: serverId,
					filter: filter && JSON.stringify({ users: filter.users, actions: filter.actions }),
				},
			})
			return {
				...page,
				users: { ...page.users, [LOCAL_CLI]: { username: 'Core CLI', avatar_url: null } },
				addons: {},
				versions: {},
				data: page.data.map((entry): Archon.Actions.v1.ActionEntry => ({
					...entry,
					actor: {
						type: 'user',
						user_id: entry.actor.type === 'user' ? entry.actor.user_id : LOCAL_CLI,
					},
					server_id: entry.server_id ?? '',
				})),
			}
		},
		async clear() {
			throw new ModrinthApiError('A Core keeps its activity log', { statusCode: 400 })
		},
	}
}
