// Port of Labrinth routes/v3/{users,friends,blocked_users,notifications}.rs (AGPL-3.0-only).
import { findUser, publicUser, requireUser } from '../common/auth'
import { randomId } from '../common/crypto'
import { ApiError, invalid, json, readBytes, readJson, routePath } from '../common/http'
import { notificationResponse, type NotificationRow } from '../common/notifications'
import type { Env, UserRow } from '../common/types'
import { blockedEitherWay, patchPreferences, preferences } from './preferences'
import { friendsStatuses, notifyUser } from './socket'

type FriendRow = { user_id: string; friend_id: string; accepted: number; created: string }
const empty = () => new Response(null, { status: 204 })
const notFound = () => new ApiError(404, 'not_found', 'Not found')

function idsQuery(url: URL): string[] {
	let ids: unknown
	try {
		ids = JSON.parse(url.searchParams.get('ids') ?? '[]')
	} catch {
		invalid('Invalid IDs')
	}
	if (
		!Array.isArray(ids) ||
		ids.length > 100 ||
		ids.some((id) => typeof id !== 'string' || id.length > 64)
	)
		invalid('Invalid IDs')
	return ids as string[]
}
function friendResponse(row: FriendRow) {
	// Labrinth deliberately returns the recipient as id and the sender as friend_id.
	return {
		id: row.friend_id,
		friend_id: row.user_id,
		accepted: Boolean(row.accepted),
		created: row.created,
	}
}
async function requireOwner(request: Request, env: Env, target: UserRow) {
	const user = await requireUser(request, env)
	if (user.id !== target.id)
		throw new ApiError(401, 'invalid_credentials', 'You do not have permission to edit this user!')
	return user
}
async function notify(env: Env, userId: string, message: unknown) {
	// Persisted social state remains authoritative if a live connection disappears.
	await notifyUser(env, userId, message).catch((error: unknown) =>
		console.error('Social notification failed', error),
	)
}

export async function handleSocial(request: Request, env: Env): Promise<Response | null> {
	const path = routePath(request),
		method = request.method,
		url = new URL(request.url)
	if (path === '/users/search' && method === 'GET') {
		const query = (url.searchParams.get('query') ?? '').trim()
		if (!query || query.length > 39) return json([])
		const prefix = query.replace(/[\\%_]/g, '\\$&') + '%'
		return json(
			(
				await env.DB.prepare(
					"SELECT id,username,avatar_url FROM users WHERE username LIKE ? ESCAPE '\\' ORDER BY username LIMIT 20",
				)
					.bind(prefix)
					.all()
			).results,
		)
	}
	if (path === '/users' && method === 'GET') {
		const users = await Promise.all(idsQuery(url).map((id) => findUser(env, id)))
		return json(
			users.filter((user): user is UserRow => user !== null).map((user) => publicUser(user)),
		)
	}
	if (path === '/friends' && method === 'GET') {
		const user = await requireUser(request, env)
		const rows = await env.DB.prepare(
			'SELECT * FROM friends WHERE user_id = ? OR friend_id = ? ORDER BY created',
		)
			.bind(user.id, user.id)
			.all<FriendRow>()
		return json(rows.results.map(friendResponse))
	}
	if (path === '/friends/status' && method === 'GET')
		return json(await friendsStatuses(env, (await requireUser(request, env)).id))
	const friendMatch = path.match(/^\/friend\/([^/]+)$/)
	if (friendMatch && ['POST', 'DELETE'].includes(method)) {
		const user = await requireUser(request, env),
			target = await findUser(env, decodeURIComponent(friendMatch[1]))
		if (!target) throw notFound()
		if (method === 'DELETE') {
			await env.DB.prepare(
				'DELETE FROM friends WHERE (user_id = ? AND friend_id = ?) OR (user_id = ? AND friend_id = ?)',
			)
				.bind(user.id, target.id, target.id, user.id)
				.run()
			await notify(env, target.id, { type: 'friend_request_rejected', from: user.id })
			await notify(env, user.id, { type: 'user_offline', id: target.id })
			await notify(env, target.id, { type: 'user_offline', id: user.id })
			return empty()
		}
		if (user.id === target.id) invalid('You cannot add yourself as a friend!')
		if (await blockedEitherWay(env, user.id, target.id))
			invalid("You've blocked the other user or have been blocked by them!")
		const existing = await env.DB.prepare(
			'SELECT * FROM friends WHERE (user_id = ? AND friend_id = ?) OR (user_id = ? AND friend_id = ?)',
		)
			.bind(user.id, target.id, target.id, user.id)
			.first<FriendRow>()
		if (existing) {
			if (existing.accepted) invalid('You are already friends with this user!')
			if (existing.friend_id !== user.id) invalid('You cannot accept your own friend request!')
			await env.DB.prepare('UPDATE friends SET accepted = 1 WHERE user_id = ? AND friend_id = ?')
				.bind(existing.user_id, existing.friend_id)
				.run()
			await Promise.all(
				[user.id, target.id].map(async (id) =>
					notify(env, id, { type: 'friend_statuses', statuses: await friendsStatuses(env, id) }),
				),
			)
		} else {
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
		return empty()
	}
	if (path === '/blocks' && method === 'GET') {
		const user = await requireUser(request, env)
		return json(
			(
				await env.DB.prepare('SELECT blocked_id FROM blocks WHERE user_id = ?')
					.bind(user.id)
					.all<{ blocked_id: string }>()
			).results.map((row) => row.blocked_id),
		)
	}
	const blockMatch = path.match(/^\/block\/([^/]+)$/)
	if (blockMatch && ['POST', 'DELETE'].includes(method)) {
		const user = await requireUser(request, env),
			target = await findUser(env, decodeURIComponent(blockMatch[1]))
		if (!target) throw notFound()
		if (target.id === user.id) invalid('You cannot block yourself')
		if (method === 'DELETE')
			await env.DB.prepare('DELETE FROM blocks WHERE user_id = ? AND blocked_id = ?')
				.bind(user.id, target.id)
				.run()
		else {
			await env.DB.batch([
				env.DB.prepare('INSERT OR IGNORE INTO blocks (user_id,blocked_id) VALUES (?,?)').bind(
					user.id,
					target.id,
				),
				env.DB.prepare(
					'DELETE FROM friends WHERE (user_id = ? AND friend_id = ?) OR (user_id = ? AND friend_id = ?)',
				).bind(user.id, target.id, target.id, user.id),
			])
			await notify(env, user.id, { type: 'user_offline', id: target.id })
			await notify(env, target.id, { type: 'user_offline', id: user.id })
		}
		return empty()
	}
	const avatarMatch = path.match(/^\/avatars\/([^/]+)\/([^/]+)$/)
	if (avatarMatch && method === 'GET') {
		const avatar = await env.DB.prepare(
			'SELECT content_type,data FROM avatars WHERE user_id = ? AND revision = ?',
		)
			.bind(avatarMatch[1], avatarMatch[2])
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
	const userMatch = path.match(
		/^\/user\/([^/]+)(?:\/(preferences|icon|notifications|projects|organizations|collections|follows|all-projects))?$/,
	)
	if (userMatch) {
		const target = await findUser(env, decodeURIComponent(userMatch[1])),
			action = userMatch[2]
		if (!target) throw notFound()
		if (!action && method === 'GET') return json(publicUser(target))
		if (
			['projects', 'organizations', 'collections', 'follows', 'all-projects'].includes(action) &&
			method === 'GET'
		) {
			// These accounts have no ownership in Modrinth's separate public-content database.
			if (action === 'follows' || action === 'all-projects')
				await requireOwner(request, env, target)
			return json(action === 'all-projects' ? { projects: [], organizations: {} } : [])
		}
		if (!action && method === 'PATCH') {
			await requireOwner(request, env, target)
			const body = await readJson(request)
			if (body.role !== undefined || body.badges !== undefined)
				throw new ApiError(403, 'forbidden', 'You cannot change account privileges')
			const username = body.username ?? target.username
			if (typeof username !== 'string' || !/^[a-zA-Z0-9._-]{1,39}$/.test(username))
				invalid('Invalid username')
			const existing = await findUser(env, username)
			if (existing && existing.id !== target.id)
				throw new ApiError(400, 'username_taken', 'Username is already taken on Modrinth.')
			const bio = body.bio === undefined ? target.bio : body.bio
			if (bio !== null && (typeof bio !== 'string' || bio.length > 160))
				invalid('Bio must be 160 characters or less')
			if (
				body.allow_friend_requests !== undefined &&
				typeof body.allow_friend_requests !== 'boolean'
			)
				invalid('Invalid friend request setting')
			await env.DB.prepare(
				'UPDATE users SET username = ?, bio = ?, allow_friend_requests = ? WHERE id = ?',
			)
				.bind(
					username,
					bio,
					body.allow_friend_requests === undefined
						? target.allow_friend_requests
						: Number(body.allow_friend_requests),
					target.id,
				)
				.run()
			return empty()
		}
		if (!action && method === 'DELETE') {
			await requireOwner(request, env, target)
			await env.DB.prepare('DELETE FROM users WHERE id = ?').bind(target.id).run()
			return empty()
		}
		if (action === 'preferences' && ['GET', 'PATCH'].includes(method)) {
			await requireOwner(request, env, target)
			if (method === 'GET') return json(preferences(target))
			const next = patchPreferences(target, await readJson(request))
			await env.DB.prepare(
				'UPDATE users SET preferences = ?, allow_friend_requests = ? WHERE id = ?',
			)
				.bind(JSON.stringify(next), next.social.friend_privacy === 'none' ? 0 : 1, target.id)
				.run()
			return json(next)
		}
		if (action === 'icon' && ['PATCH', 'DELETE'].includes(method)) {
			await requireOwner(request, env, target)
			if (method === 'DELETE') {
				await env.DB.batch([
					env.DB.prepare('DELETE FROM avatars WHERE user_id = ?').bind(target.id),
					env.DB.prepare('UPDATE users SET avatar_url = NULL WHERE id = ?').bind(target.id),
				])
				return empty()
			}
			const bytes = await readBytes(request, 262144)
			if (!bytes.length || bytes.length > 262144) invalid('Icons must be smaller than 256KiB')
			const ext = url.searchParams.get('ext')?.toLowerCase()
			const types: Record<string, string> = {
				png: 'image/png',
				jpg: 'image/jpeg',
				jpeg: 'image/jpeg',
				gif: 'image/gif',
				webp: 'image/webp',
			}
			if (!ext || !types[ext]) invalid('Unsupported image format')
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
				).bind(target.id, types[ext], btoa(binary), revision),
				env.DB.prepare('UPDATE users SET avatar_url = ? WHERE id = ?').bind(avatarUrl, target.id),
			])
			return empty()
		}
		if (action === 'notifications' && method === 'GET') {
			await requireOwner(request, env, target)
			return json(
				(
					await env.DB.prepare(
						'SELECT * FROM notifications WHERE user_id = ? ORDER BY created DESC',
					)
						.bind(target.id)
						.all<NotificationRow>()
				).results.map(notificationResponse),
			)
		}
	}
	const notificationMatch = path.match(/^\/notification\/([^/]+)$/)
	if (
		(notificationMatch || path === '/notifications') &&
		['GET', 'PATCH', 'DELETE'].includes(method)
	) {
		const user = await requireUser(request, env)
		const ids = notificationMatch ? [notificationMatch[1]] : idsQuery(url)
		if (!ids.length) return method === 'GET' ? json([]) : empty()
		const placeholders = ids.map(() => '?').join(',')
		if (method === 'GET') {
			const rows = await env.DB.prepare(
				`SELECT * FROM notifications WHERE user_id = ? AND id IN (${placeholders}) ORDER BY created DESC`,
			)
				.bind(user.id, ...ids)
				.all<NotificationRow>()
			if (notificationMatch) {
				if (!rows.results[0]) throw notFound()
				return json(notificationResponse(rows.results[0]))
			}
			return json(rows.results.map(notificationResponse))
		}
		const statement =
			method === 'PATCH' ? 'UPDATE notifications SET read = 1' : 'DELETE FROM notifications'
		await env.DB.prepare(`${statement} WHERE user_id = ? AND id IN (${placeholders})`)
			.bind(user.id, ...ids)
			.run()
		return empty()
	}
	return null
}
