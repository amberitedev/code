import type { Action, ActivityPage, CoreUser } from '@amberite/contracts'

import type { Archon } from '../modules/archon/types'

// A Core's activity in the hosting API's shapes, so the upstream server pages (`actions_v1`)
// read a Core the way they read Modrinth Hosting.

/** Shown as the actor of what the Core's local CLI did. */
const LOCAL_CLI = 'core-cli'

/**
 * One page of a server's activity as an action log. `next_offset` carries the Core's cursor, so
 * the page passes it back as `offset`. The Core lists newest first and filters only by server;
 * users, actions, and dates are filtered here, within the page.
 */
export function actionLog(
	page: ActivityPage,
	options: Archon.Actions.v1.ListActionLogOptions,
): Archon.Actions.v1.ActionLogResponse {
	const users: Archon.Actions.v1.ActionLogResponse['users'] = {
		[LOCAL_CLI]: { username: 'Core CLI' },
	}
	const min = options.min_datetime ? Date.parse(options.min_datetime) : -Infinity
	const max = options.max_datetime ? Date.parse(options.max_datetime) : Infinity
	const data = page.entries.flatMap((entry): Archon.Actions.v1.ActionEntry[] => {
		if (entry.user) users[entry.user.id] ??= userResp(entry.user)
		const actorId = entry.user?.id ?? LOCAL_CLI
		const action = hostingAction(entry.action)
		const at = entry.at * 1000
		const { filter } = options
		if (at < min || at > max) return []
		if (filter?.users?.length && !filter.users.includes(actorId)) return []
		if (filter?.actions?.length && !filter.actions.some((name) => name === action.action)) return []
		return [
			{
				actor: { type: 'user', user_id: actorId },
				action,
				server_id: entry.server_id ?? '',
				world_id: null,
				timestamp: new Date(at).toISOString(),
			},
		]
	})
	return { data, next_offset: page.next_before, users, addons: {}, versions: {} }
}

function hostingAction(action: Action): Archon.Actions.v1.Action {
	switch (action.type) {
		case 'console_command':
			return { action: 'console_command_executed', metadata: { command: action.command } }
		case 'backup_created':
		case 'backup_restored':
		case 'backup_deleted':
		case 'backup_renamed':
		case 'backup_downloaded':
			return { action: action.type, metadata: { id: action.backup_id } }
		default:
			// Server actions share their names; the rest show as they are.
			return { action: action.type }
	}
}

function userResp(user: CoreUser): Archon.ServerUsers.v1.UserResp {
	return { id: user.id, username: user.name, avatar_url: user.avatar_url }
}
