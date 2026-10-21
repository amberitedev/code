import { ModrinthApiError } from '../../../core/errors'
import type { Archon } from '../../archon/types'
import type { Copal } from '../types'

// A Core's access and activity in the hosting API's shapes, so the upstream server pages
// (`server_users_v1`, `actions_v1`) read a Core the way they read Modrinth Hosting.

/** The scopes a role reads as, in the form `apiPermissionsToAccessRole` in `@modrinth/ui` parses. */
const ROLE_SCOPES = {
	owner: 'SERVER_ADMIN',
	editor: 'BASE_READ | POWER_ACTIONS | EXEC_COMMANDS | FILES_WRITE | SETUP | BACKUPS | ADVANCED',
	viewer: 'BASE_READ',
} satisfies Record<Copal.Role | 'owner', Archon.ServerUsers.v1.UserScope>

/** Shown as the actor of what the Core's local CLI did. */
const LOCAL_CLI = 'core-cli'

/** A server's owner and members. Everyone listed has access: a Core has no pending invites. */
export function serverUsers(access: Copal.ServerAccess): Archon.ServerUsers.v1.ServerUser[] {
	return [
		...(access.owner ? [serverUser(access.owner, 'owner', null)] : []),
		...access.members.map((member) => serverUser(member.user, member.role, member.added_at)),
	]
}

/** One person with `role` on a server, given it at `addedAt` (Unix seconds) when known. */
export function serverUser(
	user: Copal.User,
	role: Copal.Role | 'owner',
	addedAt: number | null,
): Archon.ServerUsers.v1.ServerUser {
	return {
		user: userResp(user),
		added_on: addedAt === null ? null : new Date(addedAt * 1000).toISOString(),
		permissions: ROLE_SCOPES[role],
	}
}

/** For `.catch` on an owner-only Core call: null when the caller is not the owner. */
export function onlyForOwner(error: unknown): null {
	if (error instanceof ModrinthApiError && error.statusCode === 403) return null
	throw error
}

/** The Core role for a hosting role. Only editors and viewers can be given. */
export function copalRole(role: Archon.ServerUsers.v1.ServerUserRole): Copal.Role {
	if (role === 'Editor') return 'editor'
	if (role === 'Viewer') return 'viewer'
	throw new ModrinthApiError(`A Core cannot give the role ${role}`, { statusCode: 400 })
}

/**
 * One page of a server's activity as an action log. `next_offset` carries the Core's cursor, so
 * the page passes it back as `offset`. The Core lists newest first and filters only by server;
 * users, actions, and dates are filtered here, within the page.
 */
export function actionLog(
	page: Copal.ActivityPage,
	access: Copal.ServerAccess | null,
	options: Archon.Actions.v1.ListActionLogOptions,
): Archon.Actions.v1.ActionLogResponse {
	const users: Archon.Actions.v1.ActionLogResponse['users'] = {
		[LOCAL_CLI]: { username: 'Core CLI' },
	}
	for (const user of [access?.owner, ...(access?.members.map((member) => member.user) ?? [])])
		if (user) users[user.id] = userResp(user)
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

function hostingAction(action: Copal.Action): Archon.Actions.v1.Action {
	switch (action.type) {
		case 'invited':
			return {
				action: 'user_invited',
				metadata: { user_id: action.user_id, permissions: ROLE_SCOPES[action.role] },
			}
		case 'role_changed':
			return {
				action: 'user_permission_modified',
				metadata: { user_id: action.user_id, permissions: ROLE_SCOPES[action.role] },
			}
		case 'removed':
			return { action: 'user_removed', metadata: { user_id: action.user_id } }
		case 'console_command':
			return { action: 'console_command_executed', metadata: { command: action.command } }
		case 'backup_created':
		case 'backup_restored':
			return { action: action.type, metadata: { id: action.backup_id } }
		default:
			// Server actions share their names; the rest show as they are.
			return { action: action.type }
	}
}

function userResp(user: Copal.User): Archon.ServerUsers.v1.UserResp {
	return { id: user.id, username: user.name, avatar_url: user.avatar_url }
}
