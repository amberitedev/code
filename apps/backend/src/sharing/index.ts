import { requireUser } from '../common/auth'
import { randomId } from '../common/crypto'
import { ApiError, json, readBytes, readJson } from '../common/http'
import { notificationResponse } from '../common/notifications'
import type { Env } from '../common/types'
import { canInviteToSharedInstance } from '../social/preferences'
import { notifyUser } from '../social/socket'

// Limits are deliberately tight and easy to change.
const MAX_FILE_SIZE = 25 * 1024 * 1024
const MAX_CONFIG_BUNDLE_SIZE = 5 * 1024 * 1024
const MAX_VERSION_FILES = 20
const MAX_INSTANCE_SIZE = 100 * 1024 * 1024
const KEPT_VERSIONS = 5

type Instance = {
	id: string
	owner_id: string
	name: string
	icon: string | null
	quarantine: number
}
type Version = {
	instance_id: string
	version: number
	manifest: string
	ready: number
}
type File = {
	id: string
	instance_id: string
	version: number
	file_name: string
	file_type: string
	// Set once the file is uploaded. The R2 object is `<instance id>/<sha256>`.
	sha256: string | null
	size: number | null
}
type Link = { id: string; instance_id: string; expiration: string; max_uses: number; uses: number }
type Manifest = {
	modrinth_ids: string[]
	modpack_id: string | null
	game_version: string
	loader: string
	loader_version: string
}
const empty = () => new Response(null, { status: 204 })
const now = () => new Date().toISOString()
const objectKey = (instanceId: string, sha256: string) => `${instanceId}/${sha256}`
const hex = (bytes: ArrayBuffer) =>
	Array.from(new Uint8Array(bytes), (byte) => byte.toString(16).padStart(2, '0')).join('')

function string(value: unknown, field: string, max = 256): string {
	if (typeof value !== 'string' || value.length > max || !value.trim())
		throw new ApiError(400, 'invalid_input', `Invalid ${field}`)
	return value
}

function strings(value: unknown, field: string, max = 250): string[] {
	if (!Array.isArray(value) || value.length > max)
		throw new ApiError(400, 'invalid_input', `Invalid ${field}`)
	return [...new Set(value.map((item) => string(item, field)))]
}

function object(value: unknown): Record<string, unknown> {
	if (!value || typeof value !== 'object' || Array.isArray(value))
		throw new ApiError(400, 'invalid_input', 'Expected an object')
	return value as Record<string, unknown>
}

function natural(value: unknown, fallback: number, max: number): number {
	if (value === undefined) return fallback
	if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0 || value > max)
		throw new ApiError(400, 'invalid_input', 'Invalid invite limit')
	return value
}

async function instance(env: Env, id: string): Promise<Instance> {
	const result = await env.DB.prepare('SELECT * FROM shared_instances WHERE id = ?')
		.bind(id)
		.first<Instance>()
	if (!result) throw new ApiError(404, 'not_found', 'Shared instance was not found')
	return result
}

async function member(
	env: Env,
	item: Instance,
	userId: string,
	allowPending = false,
): Promise<void> {
	if (item.owner_id === userId) return
	const row = await env.DB.prepare(
		'SELECT joined_at FROM shared_members WHERE instance_id = ? AND user_id = ?',
	)
		.bind(item.id, userId)
		.first<{ joined_at: string | null }>()
	if (!row || (!allowPending && !row.joined_at))
		throw new ApiError(401, 'unauthorized', 'Shared instance access was revoked')
}

function owner(item: Instance, userId: string) {
	if (item.owner_id !== userId)
		throw new ApiError(403, 'forbidden', 'Only the owner can change this shared instance')
}

async function files(env: Env, item: Version): Promise<File[]> {
	return (
		await env.DB.prepare(
			'SELECT * FROM shared_files WHERE instance_id = ? AND version = ? ORDER BY rowid',
		)
			.bind(item.instance_id, item.version)
			.all<File>()
	).results
}

async function versionResponse(env: Env, request: Request, item: Version) {
	const origin = new URL(request.url).origin
	const manifest = JSON.parse(item.manifest) as Manifest
	return {
		...manifest,
		version: item.version,
		ready: item.ready === 1,
		// An uploaded file points at its download; one still missing points at its upload.
		external_files: (await files(env, item)).map((file) => ({
			file_name: file.file_name,
			file_type: file.file_type,
			url: file.sha256
				? `${origin}/v1/instances/${item.instance_id}/files/${file.sha256}/${encodeURIComponent(file.file_name)}`
				: `${origin}/v1/uploads/${file.id}`,
			file_size: file.size ?? undefined,
			sha256: file.sha256 ?? undefined,
		})),
	}
}

/** The newest complete version, or the one being uploaded when none is complete yet. */
async function latestVersion(env: Env, id: string): Promise<Version> {
	const result = await env.DB.prepare(
		'SELECT * FROM shared_versions WHERE instance_id = ? ORDER BY ready DESC, version DESC LIMIT 1',
	)
		.bind(id)
		.first<Version>()
	if (!result) throw new ApiError(404, 'not_found', 'Shared instance has no version')
	return result
}

/** Deletes the instance's R2 objects that no remaining version uses. */
async function sweepFiles(env: Env, id: string): Promise<void> {
	const used = new Set(
		(
			await env.DB.prepare(
				'SELECT DISTINCT sha256 FROM shared_files WHERE instance_id = ? AND sha256 IS NOT NULL',
			)
				.bind(id)
				.all<{ sha256: string }>()
		).results.map((row) => objectKey(id, row.sha256)),
	)
	const stored = await env.FILES.list({ prefix: `${id}/` })
	const unused = stored.objects.map((object) => object.key).filter((key) => !used.has(key))
	if (unused.length) await env.FILES.delete(unused)
}

/** Keeps the latest complete versions and drops uploads abandoned before the newest one. */
async function pruneVersions(env: Env, id: string): Promise<void> {
	await env.DB.prepare(
		`DELETE FROM shared_versions WHERE instance_id = ?1 AND (
			version < (SELECT MIN(version) FROM (SELECT version FROM shared_versions WHERE instance_id = ?1 AND ready = 1 ORDER BY version DESC LIMIT ?2))
			OR (ready = 0 AND version < (SELECT MAX(version) FROM shared_versions WHERE instance_id = ?1 AND ready = 1)))`,
	)
		.bind(id, KEPT_VERSIONS)
		.run()
	await sweepFiles(env, id)
}

async function notifyReadyInvites(env: Env, id: string): Promise<void> {
	const ready = await env.DB.prepare(
		'SELECT 1 FROM shared_versions WHERE instance_id = ? AND ready = 1 LIMIT 1',
	)
		.bind(id)
		.first()
	if (!ready) return
	const item = await instance(env, id)
	const pending = (
		await env.DB.prepare(
			'SELECT user_id FROM shared_members WHERE instance_id = ? AND joined_at IS NULL AND notified = 0',
		)
			.bind(id)
			.all<{ user_id: string }>()
	).results
	for (const row of pending) {
		const body = {
			type: 'shared_instance_invite',
			shared_instance_id: id,
			shared_instance_name: item.name,
			shared_instance_icon: item.icon,
			invited_by: item.owner_id,
		}
		const notification = {
			id: randomId(),
			user_id: row.user_id,
			body: JSON.stringify(body),
			created: now(),
			read: 0,
		}
		const result = await env.DB.batch([
			env.DB.prepare(
				'INSERT INTO notifications (id,user_id,body,created) SELECT ?,?,?,? WHERE EXISTS (SELECT 1 FROM shared_members WHERE instance_id = ? AND user_id = ? AND joined_at IS NULL AND notified = 0)',
			).bind(
				notification.id,
				notification.user_id,
				notification.body,
				notification.created,
				id,
				row.user_id,
			),
			env.DB.prepare(
				'UPDATE shared_members SET notified = 1 WHERE instance_id = ? AND user_id = ? AND joined_at IS NULL',
			).bind(id, row.user_id),
		])
		if (result[0]?.meta.changes)
			await notifyUser(env, row.user_id, notificationResponse(notification)).catch(() => undefined)
	}
}

/** Marks a version ready once every file it lists is uploaded. */
async function finalizeVersion(env: Env, id: string, version: number): Promise<boolean> {
	const result = await env.DB.prepare(
		`UPDATE shared_versions SET ready = 1 WHERE instance_id = ?1 AND version = ?2 AND ready = 0
		AND NOT EXISTS (SELECT 1 FROM shared_files WHERE instance_id = ?1 AND version = ?2 AND sha256 IS NULL)`,
	)
		.bind(id, version)
		.run()
	if (!result.meta.changes) return false
	await notifyReadyInvites(env, id)
	await pruneVersions(env, id)
	return true
}

async function createVersion(
	request: Request,
	env: Env,
	item: Instance,
	userId: string,
): Promise<Response> {
	owner(item, userId)
	const body = await readJson(request)
	const manifest: Manifest = {
		modrinth_ids: strings(body.modrinth_ids ?? [], 'modrinth_ids', 1000),
		modpack_id:
			body.modpack_id === undefined || body.modpack_id === null
				? null
				: string(body.modpack_id, 'modpack_id'),
		game_version: string(body.game_version, 'game_version'),
		loader: string(body.loader, 'loader'),
		loader_version: typeof body.loader_version === 'string' ? body.loader_version : '',
	}
	if (!['vanilla', 'fabric', 'forge', 'quilt', 'neoforge'].includes(manifest.loader))
		throw new ApiError(400, 'invalid_input', 'Invalid loader')
	if (!Array.isArray(body.external_files))
		throw new ApiError(400, 'invalid_input', 'Invalid external_files')
	if (body.external_files.length > MAX_VERSION_FILES)
		throw new ApiError(
			413,
			'invalid_input',
			`A version can upload at most ${MAX_VERSION_FILES} files`,
		)
	const seen = new Set<string>()
	const external = body.external_files.map((value) => {
		const file = object(value)
		const file_name = string(file.file_name, 'file_name')
		const file_type = string(file.file_type, 'file_type')
		if (/[\\/\u0000-\u001f]/.test(file_name) || file_name === '.' || file_name === '..')
			throw new ApiError(400, 'invalid_input', 'File names cannot contain paths')
		if (!['mod', 'resourcepack', 'shader', 'datapack', 'configs'].includes(file_type))
			throw new ApiError(400, 'invalid_input', 'Unsupported shared file type')
		const name = `${file_type}/${file_name}`
		if (seen.has(name)) throw new ApiError(400, 'invalid_input', 'Duplicate external file')
		seen.add(name)
		return { id: crypto.randomUUID(), file_name, file_type }
	})
	// One D1 batch allocates the monotonic version and all file rows atomically.
	const result = await env.DB.batch<Version>([
		env.DB.prepare(
			'INSERT INTO shared_versions (instance_id,version,manifest,created) SELECT ?1,COALESCE(MAX(version),0)+1,?2,?3 FROM shared_versions WHERE instance_id = ?1',
		).bind(item.id, JSON.stringify(manifest), now()),
		...external.map((file) =>
			env.DB.prepare(
				'INSERT INTO shared_files (id,instance_id,version,file_name,file_type) SELECT ?1,?2,MAX(version),?3,?4 FROM shared_versions WHERE instance_id = ?2',
			).bind(file.id, item.id, file.file_name, file.file_type),
		),
		env.DB.prepare(
			'SELECT * FROM shared_versions WHERE instance_id = ? ORDER BY version DESC LIMIT 1',
		).bind(item.id),
	])
	const version = result[result.length - 1]?.results[0]
	if (!version) throw new ApiError(500, 'database_error', 'Could not create shared version')
	const ready = await finalizeVersion(env, item.id, version.version)
	return json(await versionResponse(env, request, { ...version, ready: ready ? 1 : 0 }))
}

/** Stores one file of a version. A hash the instance already holds is not stored again. */
async function upload(request: Request, env: Env, fileId: string): Promise<Response> {
	const user = await requireUser(request, env)
	if (request.method !== 'PUT')
		throw new ApiError(405, 'method_not_allowed', 'Unsupported upload operation')
	const file = await env.DB.prepare('SELECT * FROM shared_files WHERE id = ?')
		.bind(fileId)
		.first<File>()
	if (!file) throw new ApiError(404, 'not_found', 'Upload was not found')
	const item = await instance(env, file.instance_id)
	owner(item, user.id)
	if (file.sha256) throw new ApiError(409, 'conflict', 'This file was already uploaded')
	const bytes = await readBytes(
		request,
		file.file_type === 'configs' ? MAX_CONFIG_BUNDLE_SIZE : MAX_FILE_SIZE,
	)
	const expected = request.headers.get('x-file-sha512')
	if (expected && expected !== hex(await crypto.subtle.digest('SHA-512', bytes)))
		throw new ApiError(400, 'upload_failed', 'Upload does not match its hash')
	const sha256 = hex(await crypto.subtle.digest('SHA-256', bytes))
	const stored = await env.DB.prepare(
		'SELECT COALESCE(SUM(size),0) AS size, COALESCE(MAX(sha256 = ?1),0) AS present FROM (SELECT DISTINCT sha256,size FROM shared_files WHERE instance_id = ?2 AND sha256 IS NOT NULL)',
	)
		.bind(sha256, item.id)
		.first<{ size: number; present: number }>()
	if (!stored?.present) {
		if ((stored?.size ?? 0) + bytes.byteLength > MAX_INSTANCE_SIZE)
			throw new ApiError(413, 'invalid_input', 'Shared instance storage is full')
		await env.FILES.put(objectKey(item.id, sha256), bytes)
	}
	await env.DB.prepare('UPDATE shared_files SET sha256 = ?, size = ? WHERE id = ?')
		.bind(sha256, bytes.byteLength, file.id)
		.run()
	await finalizeVersion(env, item.id, file.version)
	return empty()
}

/** Serves an uploaded file to a member, who authenticates like any other sharing request. */
async function download(
	request: Request,
	env: Env,
	item: Instance,
	sha256: string,
): Promise<Response> {
	if (item.quarantine) throw new ApiError(403, 'forbidden', 'Shared instance is quarantined')
	if (!/^[a-f0-9]{64}$/.test(sha256))
		throw new ApiError(404, 'not_found', 'Shared file was not found')
	const key = objectKey(item.id, sha256)
	const object = await env.FILES.get(key)
	if (!object) throw new ApiError(404, 'not_found', 'Shared file was not found')
	return new Response(request.method === 'HEAD' ? null : object.body, {
		headers: {
			'Content-Type': 'application/octet-stream',
			'Content-Length': String(object.size),
			'Cache-Control': 'private, no-store',
		},
	})
}

// Invite links carry this code, so it is short: 12 characters from a 32-character alphabet (60 bits).
// The look-alike characters i, l, o and 0 are left out so a code can be read aloud or typed.
const INVITE_ALPHABET = 'abcdefghjkmnpqrstuvwxyz123456789'

function inviteCode(): string {
	return Array.from(
		crypto.getRandomValues(new Uint8Array(12)),
		(byte) => INVITE_ALPHABET[byte & 31],
	).join('')
}

async function usableLink(env: Env, id: string): Promise<Link> {
	const link = await env.DB.prepare(
		'SELECT * FROM shared_links WHERE id = ? AND expiration > ? AND (max_uses = 0 OR uses < max_uses)',
	)
		.bind(id, now())
		.first<Link>()
	if (!link) throw new ApiError(404, 'not_found', 'Invite is invalid or expired')
	return link
}

async function previewInvite(env: Env, id: string): Promise<Response> {
	const link = await usableLink(env, id)
	const item = await instance(env, link.instance_id)
	const version = await latestVersion(env, item.id)
	const manifest = JSON.parse(version.manifest) as Manifest
	const manager = await env.DB.prepare('SELECT id,username,avatar_url FROM users WHERE id = ?')
		.bind(item.owner_id)
		.first<{ id: string; username: string; avatar_url: string | null }>()
	const users = (
		await env.DB.prepare(
			'SELECT u.id,u.username AS name,u.avatar_url AS avatar,m.joined_at FROM shared_members m JOIN users u ON u.id = m.user_id WHERE m.instance_id = ?',
		)
			.bind(item.id)
			.all()
	).results
	return json({
		instance_id: item.id,
		instance_name: item.name,
		instance_icon: item.icon,
		game_version: manifest.game_version,
		loader_version: manifest.loader_version,
		managers: manager
			? [{ type: 'user', id: manager.id, name: manager.username, avatar: manager.avatar_url }]
			: [],
		instance_users: users,
	})
}

async function invites(
	request: Request,
	env: Env,
	item: Instance,
	userId: string,
	inviteId?: string,
): Promise<Response> {
	if (inviteId === 'pending') {
		if (request.method === 'POST') {
			const result = await env.DB.prepare(
				'UPDATE shared_members SET joined_at = COALESCE(joined_at,?) WHERE instance_id = ? AND user_id = ?',
			)
				.bind(now(), item.id, userId)
				.run()
			if (!result.meta.changes) throw new ApiError(404, 'not_found', 'Pending invite was not found')
			return empty()
		}
		if (request.method === 'DELETE') {
			await env.DB.prepare(
				'DELETE FROM shared_members WHERE instance_id = ? AND user_id = ? AND joined_at IS NULL',
			)
				.bind(item.id, userId)
				.run()
			return empty()
		}
	}
	if (inviteId && request.method === 'POST') {
		const existing = await env.DB.prepare(
			'SELECT joined_at FROM shared_members WHERE instance_id = ? AND user_id = ?',
		)
			.bind(item.id, userId)
			.first<{ joined_at: string | null }>()
		if (existing?.joined_at) return empty()
		const link = await usableLink(env, inviteId)
		if (link.instance_id !== item.id) throw new ApiError(404, 'not_found', 'Invite was not found')
		await env.DB.batch([
			env.DB.prepare(
				"INSERT INTO shared_members (instance_id,user_id,joined_at,join_type) SELECT instance_id,?,?,'link' FROM shared_links WHERE id = ? AND expiration > ? AND (max_uses = 0 OR uses < max_uses) ON CONFLICT(instance_id,user_id) DO UPDATE SET joined_at = excluded.joined_at,join_type = excluded.join_type",
			).bind(userId, now(), inviteId, now()),
			env.DB.prepare('UPDATE shared_links SET uses = uses + 1 WHERE id = ? AND changes() > 0').bind(
				inviteId,
			),
		])
		await member(env, item, userId)
		return empty()
	}
	owner(item, userId)
	if (request.method === 'GET' && !inviteId)
		return json(
			(
				await env.DB.prepare(
					'SELECT id,expiration,max_uses,uses FROM shared_links WHERE instance_id = ? AND expiration > ? ORDER BY expiration',
				)
					.bind(item.id, now())
					.all()
			).results,
		)
	if (request.method === 'DELETE' && inviteId) {
		await env.DB.prepare('DELETE FROM shared_links WHERE id = ? AND instance_id = ?')
			.bind(inviteId, item.id)
			.run()
		return empty()
	}
	if (request.method === 'POST' && !inviteId) {
		const body = await readJson(request)
		const id = inviteCode()
		const maxAge = natural(body.max_age, 604800, 31536000)
		const maxUses = natural(body.max_uses, 0, 10000)
		const expiration = new Date(
			Date.now() + (maxAge === 0 ? 31536000 : maxAge) * 1000,
		).toISOString()
		await env.DB.prepare(
			'INSERT INTO shared_links (id,instance_id,expiration,max_uses) VALUES (?,?,?,?)',
		)
			.bind(id, item.id, expiration, maxUses)
			.run()
		return json({ id })
	}
	throw new ApiError(405, 'method_not_allowed', 'Unsupported invite operation')
}

async function users(
	request: Request,
	env: Env,
	item: Instance,
	userId: string,
): Promise<Response> {
	await member(env, item, userId)
	if (request.method === 'GET') {
		const rows = (
			await env.DB.prepare(
				'SELECT user_id AS id,joined_at,join_type,last_played FROM shared_members WHERE instance_id = ?',
			)
				.bind(item.id)
				.all()
		).results
		const tokens = await env.DB.prepare(
			'SELECT COUNT(*) AS count FROM shared_links WHERE instance_id = ? AND expiration > ? AND (max_uses = 0 OR uses < max_uses)',
		)
			.bind(item.id, now())
			.first<{ count: number }>()
		return json({ users: rows, tokens: tokens?.count ?? 0 })
	}
	const body = await readJson(request)
	const ids = strings(body.user_ids, 'user_ids', 100)
	if (request.method === 'DELETE') {
		if (ids.some((id) => id !== userId)) owner(item, userId)
		if (ids.includes(item.owner_id))
			throw new ApiError(400, 'invalid_input', 'The owner cannot leave their own instance')
		if (ids.length)
			await env.DB.batch(
				ids.map((id) =>
					env.DB.prepare('DELETE FROM shared_members WHERE instance_id = ? AND user_id = ?').bind(
						item.id,
						id,
					),
				),
			)
		return empty()
	}
	owner(item, userId)
	if (request.method !== 'POST')
		throw new ApiError(405, 'method_not_allowed', 'Unsupported member operation')
	for (const id of ids) {
		if (id === userId) continue
		if (!(await canInviteToSharedInstance(env, userId, id)))
			throw new ApiError(403, 'forbidden', 'This account does not accept this sharing invitation')
	}
	for (const id of ids) {
		if (id === userId) continue
		await env.DB.prepare(
			"INSERT OR IGNORE INTO shared_members (instance_id,user_id,join_type) VALUES (?,?,'invite')",
		)
			.bind(item.id, id)
			.run()
	}
	await notifyReadyInvites(env, item.id)
	return empty()
}

export async function handleSharing(request: Request, env: Env): Promise<Response | null> {
	const url = new URL(request.url)
	const parts = url.pathname.split('/').filter(Boolean).map(decodeURIComponent)
	if (
		parts[0] !== 'v1' ||
		!['instances', 'invites', 'uploads', 'blacklist', 'moderation', 'icons'].includes(
			parts[1] ?? '',
		)
	)
		return null
	const [, route, id, section, child] = parts
	if (route === 'invites' && id && request.method === 'GET') return previewInvite(env, id)
	if (route === 'uploads' && id && !section) return upload(request, env, id)
	if (route === 'icons' && id && request.method === 'GET') {
		const row = await env.DB.prepare(
			'SELECT icon_data,icon_type FROM shared_instances WHERE icon = ?',
		)
			.bind(url.origin + url.pathname)
			.first<{ icon_data: string | null; icon_type: string | null }>()
		if (!row?.icon_data) throw new ApiError(404, 'not_found', 'Icon was not found')
		return new Response(
			Uint8Array.from(atob(row.icon_data), (character) => character.charCodeAt(0)),
			{
				headers: {
					'Content-Type': row.icon_type ?? 'image/png',
					'Cache-Control': 'private, max-age=3600',
				},
			},
		)
	}
	const user = await requireUser(request, env)
	if (route === 'blacklist' && id && request.method === 'GET') {
		return json({
			blacklisted: !!(await env.DB.prepare('SELECT 1 FROM sharing_blacklist WHERE user_id = ?')
				.bind(id)
				.first()),
		})
	}
	if (route === 'moderation') {
		if (user.role !== 'admin' && user.role !== 'moderator')
			throw new ApiError(403, 'forbidden', 'Moderator access is required')
		if (id === 'blacklist' && ['POST', 'DELETE'].includes(request.method)) {
			const ids = strings((await readJson(request)).user_ids, 'user_ids', 100)
			if (ids.length)
				await env.DB.batch(
					ids.map((id) =>
						request.method === 'POST'
							? env.DB.prepare('INSERT OR IGNORE INTO sharing_blacklist (user_id) VALUES (?)').bind(
									id,
								)
							: env.DB.prepare('DELETE FROM sharing_blacklist WHERE user_id = ?').bind(id),
					),
				)
			return empty()
		}
		if (
			id === 'instances' &&
			section &&
			child === 'versions' &&
			parts[5] &&
			parts[6] === 'files' &&
			parts[7] &&
			request.method === 'DELETE'
		) {
			await env.DB.prepare('UPDATE shared_instances SET quarantine = 1 WHERE id = ?')
				.bind(section)
				.run()
			await env.DB.prepare(
				'DELETE FROM shared_files WHERE instance_id = ? AND version = ? AND file_name = ?',
			)
				.bind(section, Number(parts[5]), parts[7])
				.run()
			await sweepFiles(env, section)
			return empty()
		}
	}
	if (route !== 'instances') throw new ApiError(404, 'not_found', 'Sharing endpoint was not found')
	if (
		await env.DB.prepare('SELECT 1 FROM sharing_blacklist WHERE user_id = ?').bind(user.id).first()
	)
		throw new ApiError(403, 'forbidden', 'This account cannot use sharing')
	if (!id) {
		if (request.method === 'GET') {
			if (url.searchParams.has('user') && url.searchParams.get('user') !== user.id)
				throw new ApiError(403, 'forbidden', 'Private instance lists belong to their account')
			return json(
				(
					await env.DB.prepare(
						'SELECT instance_id FROM shared_members WHERE user_id = ? AND joined_at IS NOT NULL',
					)
						.bind(user.id)
						.all<{ instance_id: string }>()
				).results.map((row) => row.instance_id),
			)
		}
		if (request.method === 'POST') {
			const name = string((await readJson(request)).name, 'name')
			const instanceId = crypto.randomUUID()
			await env.DB.batch([
				env.DB.prepare(
					'INSERT INTO shared_instances (id,owner_id,name,created) VALUES (?,?,?,?)',
				).bind(instanceId, user.id, name, now()),
				env.DB.prepare(
					"INSERT INTO shared_members (instance_id,user_id,joined_at,join_type) VALUES (?,?,?,'owner')",
				).bind(instanceId, user.id, now()),
			])
			return json({ id: instanceId })
		}
		throw new ApiError(405, 'method_not_allowed', 'Unsupported instance operation')
	}
	const item = await instance(env, id)
	if (section === 'invites') return invites(request, env, item, user.id, child)
	if (section === 'users') return users(request, env, item, user.id)
	await member(env, item, user.id, !section)
	if (!section) {
		if (request.method === 'GET')
			return json({ name: item.name, icon: item.icon, quarantine: item.quarantine === 1 })
		owner(item, user.id)
		if (request.method === 'PATCH') {
			await env.DB.prepare('UPDATE shared_instances SET name = ? WHERE id = ?')
				.bind(string((await readJson(request)).name, 'name'), id)
				.run()
			return empty()
		}
		if (request.method === 'DELETE') {
			await env.DB.prepare('DELETE FROM shared_instances WHERE id = ?').bind(id).run()
			await sweepFiles(env, id)
			return empty()
		}
	}
	if (section === 'versions') {
		if (item.quarantine) throw new ApiError(403, 'forbidden', 'Shared instance is quarantined')
		if (request.method === 'POST' && !child) return createVersion(request, env, item, user.id)
		if (request.method === 'GET') {
			const version = child
				? await env.DB.prepare(
						'SELECT * FROM shared_versions WHERE instance_id = ? AND version = ?',
					)
						.bind(id, Number(child))
						.first<Version>()
				: await latestVersion(env, id)
			if (!version) throw new ApiError(404, 'not_found', 'Shared version was not found')
			return json(await versionResponse(env, request, version))
		}
	}
	if (section === 'files' && child && ['GET', 'HEAD'].includes(request.method))
		return download(request, env, item, child)
	if (section === 'icon') {
		owner(item, user.id)
		if (request.method === 'PUT') {
			const data = await readBytes(request, 256 * 1024)
			let binary = ''
			for (const byte of new Uint8Array(data)) binary += String.fromCharCode(byte)
			await env.DB.prepare(
				'UPDATE shared_instances SET icon = ?,icon_data = ?,icon_type = ? WHERE id = ?',
			)
				.bind(
					`${url.origin}/v1/icons/${crypto.randomUUID()}`,
					btoa(binary),
					request.headers.get('content-type') ?? 'image/png',
					id,
				)
				.run()
			return empty()
		}
		if (request.method === 'DELETE') {
			await env.DB.prepare(
				'UPDATE shared_instances SET icon = NULL,icon_data = NULL,icon_type = NULL WHERE id = ?',
			)
				.bind(id)
				.run()
			return empty()
		}
	}
	throw new ApiError(405, 'method_not_allowed', 'Unsupported sharing operation')
}
