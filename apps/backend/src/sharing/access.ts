import type { SharedIconConfig } from '@amberite/contracts'

import { accountLocked, requireUserWithSharing } from '../common/auth'
import { ApiError } from '../common/http'
import type { Env } from '../common/types'

// Who reaches a share, and the rows that decide it. Every sharing domain builds on these.

export type InstanceRow = {
	id: string
	owner_id: string
	name: string
	/** URL of an uploaded image, kept in R2 at `iconKey`. NULL for an icon editor icon. */
	icon: string | null
	/** JSON of the icon editor settings, which clients draw themselves. NULL for an uploaded
	 * image. See `iconConfig`. */
	icon_config: string | null
	quarantine: number
	installed_once: number
	home: string | null
	replica: string | null
	/** The Core whose key writes this share, for a server-first share; see `coreKeyRoutes`. */
	core_id: string | null
	server_id: string | null
}

export const now = () => new Date().toISOString()
export const quarantined = () => new ApiError(403, 'quarantined', 'Shared instance is quarantined')
export const blacklisted = () => new ApiError(403, 'blacklisted', 'This account cannot use sharing')

export async function instance(env: Env, id: string): Promise<InstanceRow> {
	const result = await env.DB.prepare('SELECT * FROM shared_instances WHERE id = ?')
		.bind(id)
		.first<InstanceRow>()
	if (!result) throw new ApiError(404, 'instance_not_found', 'Shared instance was not found')
	return result
}

/**
 * SQL true while the server `s` may pass its share on: its Core's owner still owns the share it
 * follows or is a joined member of it. A member who leaves or is kicked takes their servers'
 * players with them, even before `dropServers` deletes the rows.
 */
export const LIVE_SERVER = `EXISTS (SELECT 1 FROM cores lc JOIN shared_instances li ON li.id = s.instance_id WHERE lc.id = s.core_id
	AND (li.owner_id = lc.owner_id OR EXISTS (SELECT 1 FROM shared_instance_users lm WHERE lm.instance_id = li.id AND lm.user_id = lc.owner_id AND lm.joined_at IS NOT NULL)))`

/**
 * SQL joining a server's players (`p`) to the server (`s`), which names the share it follows.
 * Only servers that may pass it on (`LIVE_SERVER`), so everything players read goes through it.
 */
export const PLAYERS = `hosted_server_players p JOIN hosted_servers s ON s.core_id = p.core_id AND s.server_id = p.server_id AND ${LIVE_SERVER}`

/** Whether `userId` owns the share or has joined it as a member. Players are not members. */
export async function joinedMember(env: Env, item: InstanceRow, userId: string): Promise<boolean> {
	if (item.owner_id === userId) return true
	return !!(await env.DB.prepare(
		'SELECT 1 FROM shared_instance_users WHERE instance_id = ? AND user_id = ? AND joined_at IS NOT NULL',
	)
		.bind(item.id, userId)
		.first())
}

/**
 * Whoever reads the share: its owner, its members, and the players of every server that follows
 * it. Pending ones too with `allowPending`, so an invite can be previewed.
 */
export async function member(
	env: Env,
	item: InstanceRow,
	userId: string,
	allowPending = false,
): Promise<void> {
	if (item.owner_id === userId) return
	const row = await env.DB.prepare(
		`SELECT EXISTS (SELECT 1 FROM shared_instance_users WHERE instance_id = ?1 AND user_id = ?2 AND (?3 OR joined_at IS NOT NULL))
			OR EXISTS (SELECT 1 FROM ${PLAYERS} WHERE s.instance_id = ?1 AND p.user_id = ?2 AND (?3 OR p.joined_at IS NOT NULL)) AS reads`,
	)
		.bind(item.id, userId, allowPending ? 1 : 0)
		.first<{ reads: number }>()
	if (!row?.reads) throw new ApiError(401, 'not_member', 'Shared instance access was revoked')
}

export function owner(item: InstanceRow, userId: string) {
	if (item.owner_id !== userId)
		throw new ApiError(403, 'not_owner', 'Only the owner can change this shared instance')
}

/** The signed-in caller, refused while sharing is closed to them. */
export async function sharer(request: Request, env: Env) {
	const user = await requireUserWithSharing(request, env)
	if (user.blacklisted) throw blacklisted()
	return user
}

/**
 * Refuses `userId` while their account is locked or may not use sharing, for a caller who is not
 * that account: one read for both.
 */
export async function requireSharingAllowed(env: Env, userId: string): Promise<void> {
	const row = await env.DB.prepare(
		'SELECT EXISTS (SELECT 1 FROM user_locks WHERE user_id = ?1) AS locked, EXISTS (SELECT 1 FROM sharing_blacklist WHERE user_id = ?1) AS blacklisted',
	)
		.bind(userId)
		.first<{ locked: number; blacklisted: number }>()
	if (row?.locked) accountLocked()
	if (row?.blacklisted) throw blacklisted()
}

/** A share the caller reads, pending invitees too with `allowPending`. */
export async function readable(env: Env, id: string, userId: string, allowPending = false) {
	const item = await instance(env, id)
	await member(env, item, userId, allowPending)
	return item
}

export async function owned(env: Env, id: string, userId: string) {
	const item = await readable(env, id, userId, true)
	owner(item, userId)
	return item
}

export function iconConfig(item: InstanceRow): SharedIconConfig | null {
	return item.icon_config ? JSON.parse(item.icon_config) : null
}
