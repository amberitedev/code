import {
	type FollowedShare,
	type InstanceUser,
	type InviteLink,
	sharing,
} from '@amberite/contracts/backend'

import { ApiError } from '../common/http'
import { notifyMembers, notifyUsers } from '../common/notifications'
import { noBody, route } from '../common/router'
import type { Env } from '../common/types'
import { type InstanceRow, PLAYERS, instance, now, quarantined, sharer } from './access'
import { createLink, inviteesOf, notifyReadyInvites } from './invites'
import { object, parseFollowedShare, parseInviteLink, parseUserIds, strings } from './parse'

// A Core server's players: the people its Core's owner shares it with. They read the share the
// server follows without being its members, and join the game at the Core's address.

/** A Core server with players, and the share it follows. */
export type HostedServerRow = {
	core_id: string
	server_id: string
	instance_id: string
	name: string
	/** Its game port; players join it at the Core's address (`joinAddress`). */
	port: number
	/** `ip:port` the Core's router granted it on the internet, last known; see `joinAddress`. */
	outside: string | null
}

/**
 * Delete the servers of `ownerIds`' Cores that follow the share, with their players and links:
 * they lost the share, so their servers cannot pass it on. Statements for the caller's batch.
 */
export function dropServers(
	env: Env,
	instanceId: string,
	ownerIds: string[],
): D1PreparedStatement[] {
	const cores =
		'core_id IN (SELECT id FROM cores WHERE owner_id IN (SELECT value FROM json_each(?2)))'
	return [
		env.DB.prepare(
			`DELETE FROM shared_instance_invites WHERE instance_id = ?1 AND server_id IS NOT NULL AND ${cores}`,
		).bind(instanceId, JSON.stringify(ownerIds)),
		env.DB.prepare(`DELETE FROM hosted_servers WHERE instance_id = ?1 AND ${cores}`).bind(
			instanceId,
			JSON.stringify(ownerIds),
		),
	]
}

/**
 * A Core's server was deleted or stopped following an instance (`DELETE /core/players`): its
 * players and links go, and the share's members are told. With `shareId`, only while the server
 * still follows that share, so a late call from the Core keeps the players of a share it follows
 * since.
 */
export async function forgetServer(
	env: Env,
	coreId: string,
	serverId: string,
	shareId: string | null = null,
): Promise<void> {
	const server = await env.DB.prepare(
		'SELECT instance_id FROM hosted_servers WHERE core_id = ? AND server_id = ?',
	)
		.bind(coreId, serverId)
		.first<{ instance_id: string }>()
	if (!server || (shareId !== null && server.instance_id !== shareId)) return
	// Keyed on the share read above, so a share followed in between keeps its players and links.
	await env.DB.batch(
		['shared_instance_invites', 'hosted_servers'].map((table) =>
			env.DB.prepare(
				`DELETE FROM ${table} WHERE core_id = ? AND server_id = ? AND instance_id = ?`,
			).bind(coreId, serverId, server.instance_id),
		),
	)
	await notifyMembers(env, server.instance_id, 'shared_instance_users')
}

/**
 * Record which share a Core server follows and what it is called. The share is the server's own
 * server-first share, or one the Core's owner owns or has joined; a player cannot pass on what
 * they were given. Changing the share drops the server's players and links, as the Core does when
 * the server follows another instance: they were given the old one.
 */
async function followShare(
	env: Env,
	userId: string,
	coreId: string,
	serverId: string,
	followed: FollowedShare,
): Promise<HostedServerRow> {
	const item = await instance(env, followed.instance_id)
	const own = item.core_id === coreId && item.server_id === serverId
	const joined =
		own ||
		item.owner_id === userId ||
		(await env.DB.prepare(
			'SELECT 1 FROM shared_instance_users WHERE instance_id = ? AND user_id = ? AND joined_at IS NOT NULL',
		)
			.bind(item.id, userId)
			.first())
	if ((item.core_id !== null && !own) || !joined)
		throw new ApiError(
			403,
			'not_member',
			'A server can only follow your own share or one you joined',
		)
	if (item.quarantine) throw quarantined()
	const server: HostedServerRow = {
		core_id: coreId,
		server_id: serverId,
		instance_id: item.id,
		name: followed.name,
		port: followed.port,
		outside: followed.outside
			? `${followed.outside.ip.includes(':') ? `[${followed.outside.ip}]` : followed.outside.ip}:${followed.outside.port}`
			: null,
	}
	const current = await env.DB.prepare(
		'SELECT instance_id FROM hosted_servers WHERE core_id = ? AND server_id = ?',
	)
		.bind(coreId, serverId)
		.first<{ instance_id: string }>()
	if (current && current.instance_id !== item.id) await forgetServer(env, coreId, serverId)
	await env.DB.prepare(
		'INSERT INTO hosted_servers (core_id,server_id,instance_id,name,port,outside) VALUES (?,?,?,?,?,?) ON CONFLICT(core_id,server_id) DO UPDATE SET name = excluded.name, port = excluded.port, outside = COALESCE(excluded.outside, outside)',
	)
		.bind(coreId, serverId, item.id, server.name, server.port, server.outside)
		.run()
	return server
}

/**
 * Where players join a server: the `ip:port` its Core's router granted it, when known; otherwise
 * the host of the Core's last reported address, the one reached from outside when it has one,
 * with the server's port. Read each time, so it follows the Core's reports.
 */
export function joinAddress(
	server: HostedServerRow & { address: string; public_address: string | null },
): string {
	// The Core reports its public address on every change, so it wins over the outside address the
	// App last saw. That one is only kept for the router's port when it is still on the same IP.
	const host = (address: string) => new URL(address).hostname
	if (server.public_address) {
		const publicHost = host(server.public_address)
		return server.outside && host(`http://${server.outside}`) === publicHost
			? server.outside
			: `${publicHost}:${server.port}`
	}
	return server.outside ?? `${host(server.address)}:${server.port}`
}

/** The server a player reads the share through, with its Core's addresses; null for its owner and members. */
export async function cameThrough(env: Env, item: InstanceRow, userId: string) {
	if (item.owner_id === userId) return null
	return env.DB.prepare(
		`SELECT s.*, c.address, c.public_address FROM ${PLAYERS} JOIN cores c ON c.id = s.core_id WHERE s.instance_id = ?1 AND p.user_id = ?2 AND NOT EXISTS (SELECT 1 FROM shared_instance_users WHERE instance_id = ?1 AND user_id = ?2) ORDER BY p.joined_at IS NULL, p.joined_at LIMIT 1`,
	)
		.bind(item.id, userId)
		.first<HostedServerRow & { address: string; public_address: string | null }>()
}

/**
 * Who the caller is to a Core's server, for its players routes (the App's Play tab): the Core's
 * owner invites players, directly or by link, and removes them; so can the owner of the share the
 * server follows. The players read that share without being its members.
 */
async function serverRoles(env: Env, userId: string, coreId: string, serverId: string) {
	const core = await env.DB.prepare('SELECT owner_id FROM cores WHERE id = ?')
		.bind(coreId)
		.first<{ owner_id: string }>()
	if (!core) throw new ApiError(404, 'core_not_found', 'Core was not found')
	const server = await env.DB.prepare(
		'SELECT * FROM hosted_servers WHERE core_id = ? AND server_id = ?',
	)
		.bind(coreId, serverId)
		.first<HostedServerRow>()
	return {
		server,
		coreOwner: core.owner_id === userId,
		shareOwner: server ? (await instance(env, server.instance_id)).owner_id === userId : false,
	}
}

const notServerOwner = () => new ApiError(403, 'not_owner', "Only the server's owner can share it")
const PLAYER_LINKS =
	'FROM shared_instance_invites WHERE core_id = ? AND server_id = ? AND expiration > ?'

/** The Core's owner, refused for anyone else. */
async function requireCoreOwner(env: Env, userId: string, coreId: string, serverId: string) {
	if (!(await serverRoles(env, userId, coreId, serverId)).coreOwner) throw notServerOwner()
}

export const playerRoutes = [
	route(sharing.ListPlayers, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		const { core_id, server_id } = params
		const roles = await serverRoles(env, user.id, core_id, server_id)
		if (!roles.coreOwner && !roles.shareOwner) throw notServerOwner()
		const users = (
			await env.DB.prepare(
				'SELECT user_id AS id, joined_at, join_type, NULL AS last_played FROM hosted_server_players WHERE core_id = ? AND server_id = ? ORDER BY joined_at IS NULL, joined_at',
			)
				.bind(core_id, server_id)
				.all<InstanceUser>()
		).results
		const tokens = await env.DB.prepare(
			`SELECT COUNT(*) AS count ${PLAYER_LINKS} AND (max_uses = 0 OR uses < max_uses)`,
		)
			.bind(core_id, server_id, now())
			.first<{ count: number }>()
		return { users, tokens: tokens?.count ?? 0 }
	}),
	route(
		sharing.InvitePlayers,
		(raw) => {
			const body = object(raw)
			return { ...parseFollowedShare(body), user_ids: strings(body.user_ids, 'user_ids', 100) }
		},
		async ({ request, env, params, body }) => {
			const user = await sharer(request, env)
			const { core_id, server_id } = params
			await requireCoreOwner(env, user.id, core_id, server_id)
			const { user_ids, ...share } = await body()
			const followed = await followShare(env, user.id, core_id, server_id, share)
			// Everyone who accepts sharing invitations is invited; the rest come back in `failed`.
			const { allowed, failed } = await inviteesOf(env, user.id, user_ids)
			if (allowed.length)
				await env.DB.prepare(
					"INSERT OR IGNORE INTO hosted_server_players (core_id,server_id,user_id,join_type) SELECT ?, ?, value, 'invite' FROM json_each(?)",
				)
					.bind(core_id, server_id, JSON.stringify(allowed))
					.run()
			await notifyReadyInvites(env, followed.instance_id)
			await notifyMembers(env, followed.instance_id, 'shared_instance_users', user.id)
			return { failed }
		},
	),
	route(sharing.RemovePlayers, parseUserIds, async ({ request, env, params, body }) => {
		const user = await sharer(request, env)
		const { core_id, server_id } = params
		const roles = await serverRoles(env, user.id, core_id, server_id)
		const ids = (await body()).user_ids
		// A player may leave; the Core's owner and the share's owner remove anyone.
		if (!roles.coreOwner && !roles.shareOwner && ids.some((id) => id !== user.id))
			throw notServerOwner()
		if (ids.length)
			await env.DB.prepare(
				'DELETE FROM hosted_server_players WHERE core_id = ? AND server_id = ? AND user_id IN (SELECT value FROM json_each(?))',
			)
				.bind(core_id, server_id, JSON.stringify(ids))
				.run()
		if (roles.server) {
			const share = roles.server.instance_id
			await notifyMembers(env, share, 'shared_instance_users', user.id)
			await notifyUsers(
				env,
				ids.filter((id) => id !== user.id),
				share,
				'shared_instance_users',
			)
		}
	}),
	route(sharing.ListPlayerLinks, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		const { core_id, server_id } = params
		await requireCoreOwner(env, user.id, core_id, server_id)
		return (
			await env.DB.prepare(`SELECT id,expiration,max_uses,uses ${PLAYER_LINKS} ORDER BY expiration`)
				.bind(core_id, server_id, now())
				.all<InviteLink>()
		).results
	}),
	route(
		sharing.CreatePlayerLink,
		(raw) => {
			const body = object(raw)
			return { ...parseFollowedShare(body), ...parseInviteLink(body) }
		},
		async ({ request, env, params, body }) => {
			const user = await sharer(request, env)
			const { core_id, server_id } = params
			await requireCoreOwner(env, user.id, core_id, server_id)
			const { max_age, max_uses, ...share } = await body()
			const followed = await followShare(env, user.id, core_id, server_id, share)
			return createLink(env, { max_age, max_uses }, followed.instance_id, followed)
		},
	),
	route(sharing.DeletePlayerLink, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		const { core_id, server_id, invite_id } = params
		await requireCoreOwner(env, user.id, core_id, server_id)
		await env.DB.prepare(
			'DELETE FROM shared_instance_invites WHERE id = ? AND core_id = ? AND server_id = ?',
		)
			.bind(invite_id, core_id, server_id)
			.run()
	}),
]
