import {
	type CreatedId,
	type InstanceUser,
	type Invite,
	type InviteLink,
	type InviteUser,
	type NewInviteLink,
	type VersionManifest,
	sharing,
} from '@amberite/contracts'

import { ApiError } from '../common/http'
import {
	type NotificationRow,
	notificationResponse,
	notifyMembers,
	notifyUsers,
} from '../common/notifications'
import { noBody, route } from '../common/router'
import type { Env } from '../common/types'
import { invitableUsers } from '../social/preferences'
import { notifyUser } from '../social/socket'
import {
	type InstanceRow,
	LIVE_SERVER,
	PLAYERS,
	iconConfig,
	instance,
	joinedMember,
	member,
	now,
	owner,
	readable,
	sharer,
} from './access'
import { object, parseInviteLink, parseUserIds } from './parse'
import { type HostedServerRow, dropServers } from './servers'
import { type VersionRow, manifestText } from './versions'

// Who a share is given to: its members, invited by name or by link, and the invitations they
// answer. A server's players are invited in `servers.ts`, through the same links and notifications.

type InviteRow = {
	id: string
	instance_id: string
	expiration: string
	max_uses: number
	uses: number
	/** Set on a server's link, which makes whoever opens it a player of that server. */
	core_id: string | null
	server_id: string | null
}

/**
 * Of `ids`, the users `senderId` may invite, and the rest as `failed`. The sender is neither:
 * inviting yourself does nothing.
 */
export async function inviteesOf(env: Env, senderId: string, ids: string[]) {
	const others = ids.filter((id) => id !== senderId)
	const allowed = await invitableUsers(env, senderId, others)
	return { allowed: [...allowed], failed: others.filter((id) => !allowed.has(id)) }
}

/**
 * Once the share has a ready version, tells every pending member and player not yet told that
 * they are invited. Each notification is written with the flag that marks its invite told, in one
 * transaction, so a concurrent call sends each only once. A player is invited to the server, by
 * its Core's owner, under the server's name.
 */
export async function notifyReadyInvites(env: Env, id: string): Promise<void> {
	const ready = await env.DB.prepare(
		'SELECT 1 FROM shared_instance_versions WHERE instance_id = ? AND ready = 1 LIMIT 1',
	)
		.bind(id)
		.first()
	if (!ready) return
	const item = await instance(env, id)
	const body = (name: string, by: string) =>
		`json_object('type','shared_instance_invite','shared_instance_id',?1,'shared_instance_name',${name},'shared_instance_icon',?2,'shared_instance_icon_config',json(?3),'invited_by',${by})`
	const pendingMembers = 'instance_id = ?1 AND joined_at IS NULL AND notified = 0'
	const pendingPlayers = 's.instance_id = ?1 AND p.joined_at IS NULL AND p.notified = 0'
	const insert = (columns: string, from: string) =>
		env.DB.prepare(
			`INSERT INTO notifications (id,user_id,body,created) SELECT lower(hex(randomblob(8))),${columns},?4 FROM ${from} RETURNING *`,
		)
	const [members, players] = await env.DB.batch<NotificationRow>([
		insert(`user_id,${body('?5', '?6')}`, `shared_instance_users WHERE ${pendingMembers}`).bind(
			id,
			item.icon,
			item.icon_config,
			now(),
			item.name,
			item.owner_id,
		),
		insert(
			`p.user_id,${body('s.name', 'c.owner_id')}`,
			`${PLAYERS} JOIN cores c ON c.id = p.core_id WHERE ${pendingPlayers}`,
		).bind(id, item.icon, item.icon_config, now()),
		env.DB.prepare(`UPDATE shared_instance_users SET notified = 1 WHERE ${pendingMembers}`).bind(
			id,
		),
		env.DB.prepare(
			`UPDATE hosted_server_players SET notified = 1 WHERE (core_id, server_id, user_id) IN (SELECT p.core_id, p.server_id, p.user_id FROM ${PLAYERS} WHERE ${pendingPlayers})`,
		).bind(id),
	])
	await Promise.all(
		[...(members?.results ?? []), ...(players?.results ?? [])].map((row) =>
			notifyUser(env, row.user_id, notificationResponse(row)).catch(() => undefined),
		),
	)
}

/** Marks read the user's invites to the share, which the App replays at sign-in while unread. */
const readInvites = (env: Env, id: string, user: string) =>
	env.DB.prepare(
		"UPDATE notifications SET read = 1 WHERE user_id = ? AND read = 0 AND json_extract(body,'$.type') = 'shared_instance_invite' AND json_extract(body,'$.shared_instance_id') = ?",
	).bind(user, id)

// Invite links carry this code, so it is short: 12 characters from a 32-character alphabet (60 bits).
// The look-alike characters i, l, o and 0 are left out so a code can be read aloud or typed.
const INVITE_ALPHABET = 'abcdefghjkmnpqrstuvwxyz123456789'

function inviteCode(): string {
	return Array.from(
		crypto.getRandomValues(new Uint8Array(12)),
		(byte) => INVITE_ALPHABET[byte & 31],
	).join('')
}

/** Marks a share installed once someone other than the owner joins it. Leaving never clears it. */
async function installedOnce(env: Env, item: InstanceRow, userId: string): Promise<void> {
	if (item.installed_once || userId === item.owner_id) return
	await env.DB.prepare('UPDATE shared_instances SET installed_once = 1 WHERE id = ?')
		.bind(item.id)
		.run()
}

async function usableLink(env: Env, id: string): Promise<InviteRow> {
	const link = await env.DB.prepare(
		'SELECT * FROM shared_instance_invites WHERE id = ? AND expiration > ? AND (max_uses = 0 OR uses < max_uses)',
	)
		.bind(id, now())
		.first<InviteRow>()
	if (!link) throw new ApiError(404, 'invite_invalid', 'Invite is invalid or expired')
	return link
}

/**
 * What the link shares, shown before accepting. With `?version=true` it adds the latest ready
 * version for the install preview, without file keys or pool access, which only members get. It
 * has no version while none is ready or the instance is quarantined.
 */
async function previewInvite(env: Env, id: string, withVersion: boolean): Promise<Invite> {
	const link = await usableLink(env, id)
	const item = await instance(env, link.instance_id)
	const ready = await env.DB.prepare(
		'SELECT * FROM shared_instance_versions WHERE instance_id = ? AND ready = 1 ORDER BY version DESC LIMIT 1',
	)
		.bind(item.id)
		.first<VersionRow>()
	const manifest: VersionManifest | undefined = ready
		? JSON.parse((await manifestText(env, ready)).text)
		: undefined
	// A server's link invites to the server, by its Core's owner: it shows nothing of the
	// instance's members, whom players never see.
	const server = link.server_id
		? await env.DB.prepare(
				'SELECT s.name, c.owner_id FROM hosted_servers s JOIN cores c ON c.id = s.core_id WHERE s.core_id = ? AND s.server_id = ?',
			)
				.bind(link.core_id, link.server_id)
				.first<{ name: string; owner_id: string }>()
		: null
	if (link.server_id && !server) throw new ApiError(404, 'invite_invalid', 'Invite was not found')
	const manager = await env.DB.prepare('SELECT id,username,avatar_url FROM users WHERE id = ?')
		.bind(server?.owner_id ?? item.owner_id)
		.first<{ id: string; username: string; avatar_url: string | null }>()
	const users = server
		? []
		: (
				await env.DB.prepare(
					'SELECT u.id,u.username AS name,u.avatar_url AS avatar,m.joined_at FROM shared_instance_users m JOIN users u ON u.id = m.user_id WHERE m.instance_id = ?',
				)
					.bind(item.id)
					.all<InviteUser>()
			).results
	return {
		instance_id: item.id,
		instance_name: server?.name ?? item.name,
		instance_icon: item.icon,
		instance_icon_config: iconConfig(item),
		game_version: manifest?.game_version ?? '',
		loader_version: manifest?.loader_version ?? '',
		inviter: manager
			? { id: manager.id, name: manager.username, avatar: manager.avatar_url }
			: null,
		managers: manager
			? [{ type: 'user', id: manager.id, name: manager.username, avatar: manager.avatar_url }]
			: [],
		instance_users: users,
		...(withVersion && !item.quarantine && ready && manifest
			? {
					version: {
						...manifest,
						external_files: manifest.external_files.map(({ key: _key, ...file }) => file),
						version: ready.version,
						ready: true,
					},
				}
			: {}),
	}
}

/** A new invite link to the share `instanceId`, or with `server`, to be a player of that server. */
export async function createLink(
	env: Env,
	link: NewInviteLink,
	instanceId: string,
	server?: HostedServerRow,
): Promise<CreatedId> {
	const id = inviteCode()
	const maxAge = link.max_age ?? 604800
	const expiration = new Date(Date.now() + (maxAge === 0 ? 31536000 : maxAge) * 1000).toISOString()
	await env.DB.prepare(
		'INSERT INTO shared_instance_invites (id,instance_id,expiration,max_uses,core_id,server_id) VALUES (?,?,?,?,?,?)',
	)
		.bind(
			id,
			instanceId,
			expiration,
			link.max_uses ?? 0,
			server?.core_id ?? null,
			server?.server_id ?? null,
		)
		.run()
	return { id }
}

/** Anyone may preview an invite, signed in or not. */
export const publicInviteRoutes = [
	route(sharing.GetInvite, noBody, async ({ env, params, request }) =>
		previewInvite(env, params.id, new URL(request.url).searchParams.get('version') === 'true'),
	),
]

/** Pending routes come before the `{invite_id}` ones they would otherwise match. */
export const memberRoutes = [
	route(sharing.ListInstanceUsers, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		const item = await readable(env, params.id, user.id)
		// Players get the content, not the list of who the instance is shared with.
		if (!(await joinedMember(env, item, user.id)))
			throw new ApiError(403, 'members_only', 'Only members see who an instance is shared with')
		const rows = (
			await env.DB.prepare(
				'SELECT user_id AS id,joined_at,join_type,last_played FROM shared_instance_users WHERE instance_id = ?',
			)
				.bind(item.id)
				.all<InstanceUser>()
		).results
		const tokens = await env.DB.prepare(
			'SELECT COUNT(*) AS count FROM shared_instance_invites WHERE instance_id = ? AND server_id IS NULL AND expiration > ? AND (max_uses = 0 OR uses < max_uses)',
		)
			.bind(item.id, now())
			.first<{ count: number }>()
		return { users: rows, tokens: tokens?.count ?? 0 }
	}),
	route(sharing.InviteInstanceUsers, parseUserIds, async ({ request, env, params, body }) => {
		const user = await sharer(request, env)
		const item = await readable(env, params.id, user.id)
		owner(item, user.id)
		// Everyone who accepts sharing invitations is invited; the rest come back in `failed`.
		const { allowed, failed } = await inviteesOf(env, user.id, (await body()).user_ids)
		if (allowed.length)
			await env.DB.prepare(
				"INSERT OR IGNORE INTO shared_instance_users (instance_id,user_id,join_type) SELECT ?, value, 'invite' FROM json_each(?)",
			)
				.bind(item.id, JSON.stringify(allowed))
				.run()
		await notifyReadyInvites(env, item.id)
		await notifyMembers(env, item.id, 'shared_instance_users', user.id)
		return { failed }
	}),
	route(sharing.RemoveInstanceUsers, parseUserIds, async ({ request, env, params, body }) => {
		const user = await sharer(request, env)
		const item = await readable(env, params.id, user.id)
		const ids = (await body()).user_ids
		if (ids.some((id) => id !== user.id)) owner(item, user.id)
		if (ids.includes(item.owner_id))
			throw new ApiError(400, 'invalid_input', 'The owner cannot leave their own instance')
		if (!ids.length) return
		await env.DB.batch([
			env.DB.prepare(
				'DELETE FROM shared_instance_users WHERE instance_id = ? AND user_id IN (SELECT value FROM json_each(?))',
			).bind(item.id, JSON.stringify(ids)),
			// Leaving a share a player reads also leaves the servers it came through.
			...(ids.includes(user.id)
				? [
						env.DB.prepare(
							'DELETE FROM hosted_server_players WHERE user_id = ?2 AND (core_id, server_id) IN (SELECT core_id, server_id FROM hosted_servers WHERE instance_id = ?1)',
						).bind(item.id, user.id),
					]
				: []),
			// A member who leaves or is kicked can no longer pass the share on through their servers.
			...dropServers(env, item.id, ids),
		])
		await notifyMembers(env, item.id, 'shared_instance_users', user.id)
		await notifyUsers(
			env,
			ids.filter((id) => id !== user.id),
			item.id,
			'shared_instance_users',
		)
	}),
	// A server's links are listed with its players.
	route(sharing.ListInvites, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		owner(await instance(env, params.id), user.id)
		return (
			await env.DB.prepare(
				'SELECT id,expiration,max_uses,uses FROM shared_instance_invites WHERE instance_id = ? AND server_id IS NULL AND expiration > ? ORDER BY expiration',
			)
				.bind(params.id, now())
				.all<InviteLink>()
		).results
	}),
	route(
		sharing.CreateInvite,
		(raw) => parseInviteLink(object(raw)),
		async ({ request, env, params, body }) => {
			const user = await sharer(request, env)
			owner(await instance(env, params.id), user.id)
			return createLink(env, await body(), params.id)
		},
	),
	// The invite to the share may have come as a member or as a player of a server following it;
	// accepting or declining answers every pending one.
	route(sharing.AcceptPendingInvite, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		const item = await instance(env, params.id)
		const [members, players] = await env.DB.batch([
			env.DB.prepare(
				'UPDATE shared_instance_users SET joined_at = COALESCE(joined_at,?1) WHERE instance_id = ?2 AND user_id = ?3',
			).bind(now(), item.id, user.id),
			env.DB.prepare(
				'UPDATE hosted_server_players SET joined_at = COALESCE(joined_at,?1) WHERE user_id = ?3 AND (core_id, server_id) IN (SELECT core_id, server_id FROM hosted_servers WHERE instance_id = ?2)',
			).bind(now(), item.id, user.id),
			readInvites(env, item.id, user.id),
		])
		if (!members?.meta.changes && !players?.meta.changes)
			throw new ApiError(404, 'invite_not_found', 'Pending invite was not found')
		await installedOnce(env, item, user.id)
		await notifyMembers(env, item.id, 'shared_instance_users', user.id)
	}),
	route(sharing.DeclinePendingInvite, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		const item = await instance(env, params.id)
		const [members, players] = await env.DB.batch([
			env.DB.prepare(
				'DELETE FROM shared_instance_users WHERE instance_id = ? AND user_id = ? AND joined_at IS NULL',
			).bind(item.id, user.id),
			env.DB.prepare(
				'DELETE FROM hosted_server_players WHERE user_id = ?2 AND joined_at IS NULL AND (core_id, server_id) IN (SELECT core_id, server_id FROM hosted_servers WHERE instance_id = ?1)',
			).bind(item.id, user.id),
			readInvites(env, item.id, user.id),
		])
		if (members?.meta.changes || players?.meta.changes)
			await notifyMembers(env, item.id, 'shared_instance_users', user.id)
	}),
	route(sharing.AcceptInvite, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		const item = await instance(env, params.id)
		const inviteId = params.invite_id
		const link = await env.DB.prepare('SELECT * FROM shared_instance_invites WHERE id = ?')
			.bind(inviteId)
			.first<InviteRow>()
		// A server's link makes them a player of that server; any other makes them a member.
		const joined = link?.server_id
			? await env.DB.prepare(
					'SELECT joined_at FROM hosted_server_players WHERE core_id = ? AND server_id = ? AND user_id = ?',
				)
					.bind(link.core_id, link.server_id, user.id)
					.first<{ joined_at: string | null }>()
			: await env.DB.prepare(
					'SELECT joined_at FROM shared_instance_users WHERE instance_id = ? AND user_id = ?',
				)
					.bind(item.id, user.id)
					.first<{ joined_at: string | null }>()
		if (joined?.joined_at) return
		const usable = await usableLink(env, inviteId)
		if (usable.instance_id !== item.id)
			throw new ApiError(404, 'invite_invalid', 'Invite was not found')
		await env.DB.batch([
			usable.server_id
				? env.DB.prepare(
						`INSERT INTO hosted_server_players (core_id,server_id,user_id,joined_at,join_type) SELECT l.core_id,l.server_id,?1,?2,'link' FROM shared_instance_invites l JOIN hosted_servers s ON s.core_id = l.core_id AND s.server_id = l.server_id AND s.instance_id = l.instance_id WHERE ${LIVE_SERVER} AND l.id = ?3 AND l.expiration > ?2 AND (l.max_uses = 0 OR l.uses < l.max_uses) ON CONFLICT(core_id,server_id,user_id) DO UPDATE SET joined_at = excluded.joined_at,join_type = excluded.join_type`,
					).bind(user.id, now(), inviteId)
				: env.DB.prepare(
						"INSERT INTO shared_instance_users (instance_id,user_id,joined_at,join_type) SELECT instance_id,?,?,'link' FROM shared_instance_invites WHERE id = ? AND expiration > ? AND (max_uses = 0 OR uses < max_uses) ON CONFLICT(instance_id,user_id) DO UPDATE SET joined_at = excluded.joined_at,join_type = excluded.join_type",
					).bind(user.id, now(), inviteId, now()),
			env.DB.prepare(
				'UPDATE shared_instance_invites SET uses = uses + 1 WHERE id = ? AND changes() > 0',
			).bind(inviteId),
			readInvites(env, item.id, user.id),
		])
		await member(env, item, user.id)
		await installedOnce(env, item, user.id)
		await notifyMembers(env, item.id, 'shared_instance_users', user.id)
	}),
	route(sharing.DeleteInvite, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		owner(await instance(env, params.id), user.id)
		await env.DB.prepare('DELETE FROM shared_instance_invites WHERE id = ? AND instance_id = ?')
			.bind(params.invite_id, params.id)
			.run()
	}),
]
