// Port of Labrinth routes/v3/{users,friends,blocked_users,notifications}.rs (AGPL-3.0-only).
import {
	type FriendsSocketMessage,
	type Notification,
	type User,
	type UserFriend,
	type UserUpdate,
	social,
} from '@amberite/contracts'

import { accountLock, findUser, publicUser, requireUser } from '../common/auth'
import { randomId } from '../common/crypto'
import { ApiError, invalid, objectBody } from '../common/http'
import { notificationResponse, type NotificationRow } from '../common/notifications'
import { type Route, noBody, rawBody, route } from '../common/router'
import type { Env, UserRow } from '../common/types'
import { sweepFiles } from '../sharing'
import {
	blockedEitherWay,
	parsePreferencesUpdate,
	patchPreferences,
	preferences,
} from './preferences'
import { friendsSocket, friendsStatuses, notifyUser } from './socket'

/** The largest avatar a user may upload. Avatars are kept in D1, so this stays well under a row. */
export const USER_ICON_BYTES = 512 * 1024

type FriendRow = { user_id: string; friend_id: string; accepted: number; created: string }
const notFound = () => new ApiError(404, 'not_found', 'Not found')

function idsQuery(request: Request): string[] {
	let ids: unknown
	try {
		ids = JSON.parse(new URL(request.url).searchParams.get('ids') ?? '[]')
	} catch {
		invalid('Invalid IDs')
	}
	if (!Array.isArray(ids) || ids.length > 100) invalid('Invalid IDs')
	const list: unknown[] = ids
	if (!list.every((id): id is string => typeof id === 'string' && id.length <= 64))
		invalid('Invalid IDs')
	return list
}
function friendResponse(row: FriendRow): UserFriend {
	// Labrinth deliberately returns the recipient as id and the sender as friend_id.
	return {
		id: row.friend_id,
		friend_id: row.user_id,
		accepted: Boolean(row.accepted),
		created: row.created,
	}
}
async function requireTarget(env: Env, id: string) {
	const target = await findUser(env, id)
	if (!target) throw notFound()
	return target
}
async function requireOwner(request: Request, env: Env, target: UserRow) {
	const user = await requireUser(request, env)
	if (user.id !== target.id)
		throw new ApiError(401, 'invalid_credentials', 'You do not have permission to edit this user!')
	return user
}
/** The target of a route only its owner may call, once the caller is checked to be them. */
async function ownTarget(request: Request, env: Env, id: string) {
	const target = await requireTarget(env, id)
	await requireOwner(request, env, target)
	return target
}
async function notify(env: Env, userId: string, ...messages: FriendsSocketMessage[]) {
	// Persisted social state remains authoritative if a live connection disappears.
	await notifyUser(env, userId, ...messages).catch((error: unknown) =>
		console.error('Social notification failed', error),
	)
}

/**
 * `userId` ended a friendship or request with `targetId`. The target's App drops them from its
 * list and presence; this account's App already took the target off its list and only forgets
 * the target's presence.
 */
async function unfriended(env: Env, userId: string, targetId: string) {
	await Promise.all([
		notify(env, targetId, { type: 'friend_request_rejected', from: userId }),
		notify(env, userId, { type: 'user_offline', id: targetId }),
	])
}

/** The signed-in account looking at a profile, or null for a visitor. */
async function viewerOf(request: Request, env: Env) {
	return requireUser(request, env).catch((error: unknown) => {
		if (error instanceof ApiError && [401, 403].includes(error.status)) return null
		throw error
	})
}

async function visibleUser(env: Env, viewer: UserRow | null, target: UserRow): Promise<User> {
	const staff = viewer?.role === 'admin' || viewer?.role === 'moderator'
	const lock = staff ? await accountLock(env, target.id) : null
	return { ...publicUser(target, viewer?.role === 'admin'), ...(lock ? { lock } : {}) }
}

/**
 * Sends a friend request, or accepts the one `target` sent. The refusals keep labrinth's text
 * byte for byte: upstream's access page matches it.
 */
async function addFriend(env: Env, user: UserRow, target: UserRow) {
	if (user.id === target.id)
		throw new ApiError(400, 'cannot_friend_self', 'You cannot add yourself as a friend!')
	if (await blockedEitherWay(env, user.id, target.id))
		invalid("You've blocked the other user or have been blocked by them!")
	const existing = await env.DB.prepare(
		'SELECT * FROM friends WHERE (user_id = ? AND friend_id = ?) OR (user_id = ? AND friend_id = ?)',
	)
		.bind(user.id, target.id, target.id, user.id)
		.first<FriendRow>()
	if (existing) {
		if (existing.accepted)
			throw new ApiError(400, 'already_friends', 'You are already friends with this user!')
		if (existing.friend_id !== user.id)
			throw new ApiError(400, 'own_friend_request', 'You cannot accept your own friend request!')
		await env.DB.prepare('UPDATE friends SET accepted = 1 WHERE user_id = ? AND friend_id = ?')
			.bind(existing.user_id, existing.friend_id)
			.run()
		// Upstream sends each side only the other's presence. The request's sender is also told
		// of the accept, which they would otherwise only see on their next friends list fetch.
		const [mine, theirs] = await Promise.all(
			[user.id, target.id].map((id) => env.FRIENDS.getByName(id).status()),
		)
		const accepted: FriendsSocketMessage[] = [{ type: 'friend_request', from: user.id }]
		if (mine) accepted.push({ type: 'status_update', status: mine })
		await Promise.all([
			notify(env, target.id, ...accepted),
			theirs && notify(env, user.id, { type: 'status_update', status: theirs }),
		])
		return
	}
	const privacy = preferences(target).social.friend_privacy
	if (!target.allow_friend_requests || privacy === 'none')
		invalid('Friend requests are disabled for this user!')
	if (privacy === 'mutual') {
		const mutual = await env.DB.prepare(
			'SELECT 1 FROM friends a JOIN friends b ON (CASE WHEN a.user_id = ? THEN a.friend_id ELSE a.user_id END) = (CASE WHEN b.user_id = ? THEN b.friend_id ELSE b.user_id END) WHERE a.accepted = 1 AND b.accepted = 1 AND (a.user_id = ? OR a.friend_id = ?) AND (b.user_id = ? OR b.friend_id = ?) LIMIT 1',
		)
			.bind(user.id, target.id, user.id, user.id, target.id, target.id)
			.first()
		if (!mutual) invalid('This user only accepts requests from mutual friends!')
	}
	const result = await env.DB.prepare(
		'INSERT OR IGNORE INTO friends (user_id,friend_id,created) VALUES (?,?,?)',
	)
		.bind(user.id, target.id, new Date().toISOString())
		.run()
	if (result.meta.changes !== 1) invalid('A friend request already exists. Please try again.')
	await notify(env, target.id, { type: 'friend_request', from: user.id })
}

async function blockUser(env: Env, user: UserRow, target: UserRow) {
	const [, unfriend] = await env.DB.batch([
		env.DB.prepare('INSERT OR IGNORE INTO blocks (user_id,blocked_id) VALUES (?,?)').bind(
			user.id,
			target.id,
		),
		env.DB.prepare(
			'DELETE FROM friends WHERE (user_id = ? AND friend_id = ?) OR (user_id = ? AND friend_id = ?)',
		).bind(user.id, target.id, target.id, user.id),
	])
	if (unfriend?.meta.changes) await unfriended(env, user.id, target.id)
}

/** The account a block or unblock is about, which cannot be the caller. */
async function blockTarget(request: Request, env: Env, id: string) {
	const user = await requireUser(request, env)
	const target = await requireTarget(env, id)
	if (target.id === user.id) invalid('You cannot block yourself')
	return { user, target }
}

async function avatar(env: Env, userId: string, revision: string) {
	const avatar = await env.DB.prepare(
		'SELECT content_type,data FROM avatars WHERE user_id = ? AND revision = ?',
	)
		.bind(userId, revision)
		.first<{ content_type: string; data: string }>()
	if (!avatar) throw notFound()
	const bytes = Uint8Array.from(atob(avatar.data), (char) => char.charCodeAt(0))
	return new Response(bytes, {
		headers: {
			'Content-Type': avatar.content_type,
			'Cache-Control': 'public, max-age=86400',
			'X-Content-Type-Options': 'nosniff',
		},
	})
}

const ICON_TYPES: Record<string, string> = {
	png: 'image/png',
	jpg: 'image/jpeg',
	jpeg: 'image/jpeg',
	gif: 'image/gif',
	webp: 'image/webp',
}

async function changeAvatar(request: Request, env: Env, target: UserRow, image: Blob) {
	const bytes = new Uint8Array(await image.arrayBuffer())
	if (!bytes.length || bytes.length > USER_ICON_BYTES) invalid('Icons must be smaller than 512KiB')
	const url = new URL(request.url)
	const ext = url.searchParams.get('ext')?.toLowerCase()
	if (!ext || !ICON_TYPES[ext]) invalid('Unsupported image format')
	const valid =
		ext === 'png'
			? bytes[0] === 137 && bytes[1] === 80 && bytes[2] === 78 && bytes[3] === 71
			: ext === 'jpg' || ext === 'jpeg'
				? bytes[0] === 255 && bytes[1] === 216 && bytes[2] === 255
				: ext === 'gif'
					? String.fromCharCode(...bytes.slice(0, 6)).match(/^GIF8[79]a$/)
					: String.fromCharCode(...bytes.slice(0, 4)) === 'RIFF' &&
						String.fromCharCode(...bytes.slice(8, 12)) === 'WEBP'
	if (!valid) invalid('Invalid image')
	let binary = ''
	for (let i = 0; i < bytes.length; i += 8192)
		binary += String.fromCharCode(...bytes.subarray(i, i + 8192))
	const revision = randomId(),
		avatarUrl = `${url.origin}/avatars/${target.id}/${revision}`
	await env.DB.batch([
		env.DB.prepare(
			'INSERT INTO avatars (user_id,content_type,data,revision) VALUES (?,?,?,?) ON CONFLICT(user_id) DO UPDATE SET content_type=excluded.content_type,data=excluded.data,revision=excluded.revision',
		).bind(target.id, ICON_TYPES[ext], btoa(binary), revision),
		env.DB.prepare('UPDATE users SET avatar_url = ? WHERE id = ?').bind(avatarUrl, target.id),
	])
}

/** `raw` as the body of `EditUser`. Role and badges are not the account's to change. */
function parseUserUpdate(raw: unknown): UserUpdate {
	const body = objectBody(raw)
	if (body.role !== undefined || body.badges !== undefined)
		throw new ApiError(403, 'forbidden', 'You cannot change account privileges')
	const { bio, allow_friend_requests: allowFriendRequests } = body
	const username = body.username ?? undefined
	if (
		username !== undefined &&
		(typeof username !== 'string' || !/^[a-zA-Z0-9._-]{1,39}$/.test(username))
	)
		invalid('Invalid username')
	if (bio !== undefined && bio !== null && (typeof bio !== 'string' || bio.length > 160))
		invalid('Bio must be 160 characters or less')
	if (allowFriendRequests !== undefined && typeof allowFriendRequests !== 'boolean')
		invalid('Invalid friend request setting')
	return { username, bio, allow_friend_requests: allowFriendRequests }
}

async function editUser(env: Env, target: UserRow, update: UserUpdate) {
	const username = update.username ?? target.username
	const existing = await findUser(env, username)
	if (existing && existing.id !== target.id)
		throw new ApiError(400, 'username_taken', 'Username is already taken on Modrinth.')
	await env.DB.prepare(
		'UPDATE users SET username = ?, bio = ?, allow_friend_requests = ? WHERE id = ?',
	)
		.bind(
			username,
			update.bio === undefined ? target.bio : update.bio,
			update.allow_friend_requests === undefined
				? target.allow_friend_requests
				: Number(update.allow_friend_requests),
			target.id,
		)
		.run()
}

/**
 * The owner deletes the account. It cascades to its shared instances, Cores and their rows, but
 * not to the files those instances hold in R2.
 */
async function deleteUser(env: Env, target: UserRow) {
	const instances = (
		await env.DB.prepare('SELECT id FROM shared_instances WHERE owner_id = ?')
			.bind(target.id)
			.all<{ id: string }>()
	).results
	await env.DB.prepare('DELETE FROM users WHERE id = ?').bind(target.id).run()
	await env.FRIENDS.getByName(target.id).revalidateSessions()
	for (const instance of instances) await sweepFiles(env, instance.id)
}

/** The caller's notifications among `ids`, newest first. */
async function notifications(env: Env, userId: string, ids: string[]): Promise<Notification[]> {
	if (!ids.length) return []
	const rows = await env.DB.prepare(
		`SELECT * FROM notifications WHERE user_id = ? AND id IN (${ids.map(() => '?').join(',')}) ORDER BY created DESC`,
	)
		.bind(userId, ...ids)
		.all<NotificationRow>()
	return rows.results.map(notificationResponse)
}

/** Marks the caller's notifications among `ids` read, or deletes them. */
async function changeNotifications(
	env: Env,
	userId: string,
	ids: string[],
	change: 'read' | 'delete',
) {
	if (!ids.length) return
	const statement =
		change === 'read' ? 'UPDATE notifications SET read = 1' : 'DELETE FROM notifications'
	await env.DB.prepare(`${statement} WHERE user_id = ? AND id IN (${ids.map(() => '?').join(',')})`)
		.bind(userId, ...ids)
		.run()
}

export const socialRoutes: Route[] = [
	route(social.SearchUsers, noBody, async ({ request, env }) => {
		const query = (new URL(request.url).searchParams.get('query') ?? '').trim()
		if (!query || query.length > 39) return []
		const prefix = query.replace(/[\\%_]/g, '\\$&') + '%'
		return (
			await env.DB.prepare(
				"SELECT id,username,avatar_url FROM users WHERE username LIKE ? ESCAPE '\\' ORDER BY username LIMIT 20",
			)
				.bind(prefix)
				.all<{ id: string; username: string; avatar_url: string | null }>()
		).results
	}),
	route(social.GetUsers, noBody, async ({ request, env }) => {
		const ids = idsQuery(request)
		if (!ids.length) return []
		// Each App asks for every user it shows here, so this is one query whatever the count.
		const [viewer, users] = await Promise.all([
			viewerOf(request, env),
			env.DB.prepare(
				'SELECT * FROM users WHERE id IN (SELECT value FROM json_each(?1)) OR username IN (SELECT value FROM json_each(?1))',
			)
				.bind(JSON.stringify(ids))
				.all<UserRow>(),
		])
		return Promise.all(users.results.map((user) => visibleUser(env, viewer, user)))
	}),
	route(social.ListFriends, noBody, async ({ request, env }) => {
		const user = await requireUser(request, env)
		const rows = await env.DB.prepare(
			'SELECT * FROM friends WHERE user_id = ? OR friend_id = ? ORDER BY created',
		)
			.bind(user.id, user.id)
			.all<FriendRow>()
		return rows.results.map(friendResponse)
	}),
	route(social.ListFriendStatuses, noBody, async ({ request, env }) =>
		friendsStatuses(env, (await requireUser(request, env)).id),
	),
	route(social.AddFriend, noBody, async ({ request, env, params }) => {
		const user = await requireUser(request, env)
		await addFriend(env, user, await requireTarget(env, params.id))
	}),
	route(social.RemoveFriend, noBody, async ({ request, env, params }) => {
		const user = await requireUser(request, env)
		const target = await requireTarget(env, params.id)
		const deleted = await env.DB.prepare(
			'DELETE FROM friends WHERE (user_id = ? AND friend_id = ?) OR (user_id = ? AND friend_id = ?)',
		)
			.bind(user.id, target.id, target.id, user.id)
			.run()
		if (deleted.meta.changes) await unfriended(env, user.id, target.id)
	}),
	route(social.ListBlocks, noBody, async ({ request, env }) => {
		const user = await requireUser(request, env)
		return (
			await env.DB.prepare('SELECT blocked_id FROM blocks WHERE user_id = ?')
				.bind(user.id)
				.all<{ blocked_id: string }>()
		).results.map((row) => row.blocked_id)
	}),
	route(social.BlockUser, noBody, async ({ request, env, params }) => {
		const { user, target } = await blockTarget(request, env, params.id)
		await blockUser(env, user, target)
	}),
	route(social.UnblockUser, noBody, async ({ request, env, params }) => {
		const { user, target } = await blockTarget(request, env, params.id)
		await env.DB.prepare('DELETE FROM blocks WHERE user_id = ? AND blocked_id = ?')
			.bind(user.id, target.id)
			.run()
	}),
	route(social.GetAvatar, noBody, async ({ env, params }) =>
		avatar(env, params.user_id, params.revision),
	),
	route(social.GetUser, noBody, async ({ request, env, params }) => {
		const target = await requireTarget(env, params.id)
		return visibleUser(env, await viewerOf(request, env), target)
	}),
	// These accounts have no ownership in Modrinth's separate public-content database.
	route(social.ListUserProjects, noBody, async ({ env, params }) => {
		await requireTarget(env, params.id)
		return []
	}),
	route(social.ListUserOrganizations, noBody, async ({ env, params }) => {
		await requireTarget(env, params.id)
		return []
	}),
	route(social.ListUserCollections, noBody, async ({ env, params }) => {
		await requireTarget(env, params.id)
		return []
	}),
	route(social.ListFollowedProjects, noBody, async ({ request, env, params }) => {
		await ownTarget(request, env, params.id)
		return []
	}),
	route(social.ListAllProjects, noBody, async ({ request, env, params }) => {
		await ownTarget(request, env, params.id)
		return { projects: [], organizations: {} }
	}),
	route(social.EditUser, parseUserUpdate, async ({ request, env, params, body }) => {
		const target = await ownTarget(request, env, params.id)
		await editUser(env, target, await body())
	}),
	route(social.DeleteUser, noBody, async ({ request, env, params }) =>
		deleteUser(env, await ownTarget(request, env, params.id)),
	),
	route(social.GetPreferences, noBody, async ({ request, env, params }) =>
		preferences(await ownTarget(request, env, params.id)),
	),
	route(social.EditPreferences, parsePreferencesUpdate, async ({ request, env, params, body }) => {
		const target = await ownTarget(request, env, params.id)
		const next = patchPreferences(target, await body())
		await env.DB.prepare('UPDATE users SET preferences = ?, allow_friend_requests = ? WHERE id = ?')
			.bind(JSON.stringify(next), next.social.friend_privacy === 'none' ? 0 : 1, target.id)
			.run()
		return next
	}),
	route(
		social.ChangeAvatar,
		rawBody,
		async ({ request, env, params, body }) => {
			const target = await ownTarget(request, env, params.id)
			await changeAvatar(request, env, target, await body())
		},
		USER_ICON_BYTES,
	),
	route(social.RemoveAvatar, noBody, async ({ request, env, params }) => {
		const target = await ownTarget(request, env, params.id)
		await env.DB.batch([
			env.DB.prepare('DELETE FROM avatars WHERE user_id = ?').bind(target.id),
			env.DB.prepare('UPDATE users SET avatar_url = NULL WHERE id = ?').bind(target.id),
		])
	}),
	route(social.ListUserNotifications, noBody, async ({ request, env, params }) => {
		const target = await ownTarget(request, env, params.id)
		return (
			await env.DB.prepare('SELECT * FROM notifications WHERE user_id = ? ORDER BY created DESC')
				.bind(target.id)
				.all<NotificationRow>()
		).results.map(notificationResponse)
	}),
	route(social.GetNotification, noBody, async ({ request, env, params }) => {
		const user = await requireUser(request, env)
		const [notification] = await notifications(env, user.id, [params.id])
		if (!notification) throw notFound()
		return notification
	}),
	route(social.ReadNotification, noBody, async ({ request, env, params }) =>
		changeNotifications(env, (await requireUser(request, env)).id, [params.id], 'read'),
	),
	route(social.DeleteNotification, noBody, async ({ request, env, params }) =>
		changeNotifications(env, (await requireUser(request, env)).id, [params.id], 'delete'),
	),
	route(social.GetNotifications, noBody, async ({ request, env }) => {
		const user = await requireUser(request, env)
		return notifications(env, user.id, idsQuery(request))
	}),
	route(social.ReadNotifications, noBody, async ({ request, env }) => {
		const user = await requireUser(request, env)
		await changeNotifications(env, user.id, idsQuery(request), 'read')
	}),
	route(social.DeleteNotifications, noBody, async ({ request, env }) => {
		const user = await requireUser(request, env)
		await changeNotifications(env, user.id, idsQuery(request), 'delete')
	}),
	route(social.FriendsSocket, noBody, async ({ request, env }) => friendsSocket(request, env)),
]
