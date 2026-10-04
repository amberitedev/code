import { requireUser } from '../common/auth'
import { randomId } from '../common/crypto'
import { ApiError, json, readBytes, readJson } from '../common/http'
import { notificationResponse } from '../common/notifications'
import type { Env } from '../common/types'
import { canInviteToSharedInstance } from '../social/preferences'
import { notifyUser } from '../social/socket'
import {
	acceptStorageReceipt,
	downloadBlob,
	onlineNodes,
	prepareUpload,
	storageHeartbeat,
	uploadUrl,
	validHash,
	validSize,
	verifiedReplica,
	type StoredBlob,
} from './storage'

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
	request_hash: string
}
type File = {
	id: string
	instance_id: string
	version: number
	file_name: string
	file_type: string
	blob_id: string | null
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

async function blob(env: Env, id: string): Promise<StoredBlob> {
	const result = await env.DB.prepare('SELECT * FROM shared_blobs WHERE id = ?')
		.bind(id)
		.first<StoredBlob>()
	if (!result) throw new ApiError(503, 'storage_unavailable', 'Shared content is unavailable')
	return result
}

async function versionResponse(
	env: Env,
	request: Request,
	item: Version,
	userId: string,
	uploading = false,
) {
	const origin = new URL(request.url).origin
	const external = []
	for (const file of await files(env, item)) {
		const stored = file.blob_id ? await blob(env, file.blob_id) : null
		let url = `${origin}/v1/uploads/${file.id}`
		if (!uploading && stored) {
			const token = crypto.randomUUID()
			await env.DB.prepare(
				'INSERT INTO shared_downloads (token,file_id,user_id,expires) VALUES (?,?,?,?)',
			)
				.bind(token, file.id, userId, Date.now() + 24 * 60 * 60 * 1000)
				.run()
			url = `${origin}/v1/downloads/${token}/${encodeURIComponent(file.file_name)}`
		}
		external.push({
			file_name: file.file_name,
			file_type: file.file_type,
			url,
			file_size: stored?.size,
			sha256: stored?.sha256,
		})
	}
	const manifest = JSON.parse(item.manifest) as Manifest
	return { ...manifest, version: item.version, ready: item.ready === 1, external_files: external }
}

async function latestVersion(env: Env, id: string): Promise<Version> {
	const result = await env.DB.prepare(
		'SELECT * FROM shared_versions WHERE instance_id = ? ORDER BY version DESC LIMIT 1',
	)
		.bind(id)
		.first<Version>()
	if (!result) throw new ApiError(404, 'not_found', 'Shared instance has no version')
	return result
}

async function pruneVersions(env: Env, id: string): Promise<void> {
	await env.DB.prepare(
		'DELETE FROM shared_versions WHERE instance_id = ? AND ready = 1 AND pinned = 0 AND version NOT IN (SELECT version FROM shared_versions WHERE instance_id = ? AND ready = 1 ORDER BY version DESC LIMIT 5)',
	)
		.bind(id, id)
		.run()
	// Nodes retain bytes until a later coordinated garbage-collection pass; no deletion during outages.
	await env.DB.prepare('DELETE FROM shared_downloads WHERE expires < ?').bind(Date.now()).run()
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

async function finalizeVersion(env: Env, fileId: string): Promise<void> {
	const file = await env.DB.prepare('SELECT instance_id,version FROM shared_files WHERE id = ?')
		.bind(fileId)
		.first<File>()
	if (!file) return
	await env.DB.prepare(
		`UPDATE shared_versions SET ready = 1 WHERE instance_id = ? AND version = ?
		AND NOT EXISTS (SELECT 1 FROM shared_files f WHERE f.instance_id = ? AND f.version = ?
		AND (f.blob_id IS NULL OR NOT EXISTS (SELECT 1 FROM shared_replicas r WHERE r.blob_id = f.blob_id)))`,
	)
		.bind(file.instance_id, file.version, file.instance_id, file.version)
		.run()
	await notifyReadyInvites(env, file.instance_id)
	await pruneVersions(env, file.instance_id)
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
	if (!Array.isArray(body.external_files) || body.external_files.length > 250)
		throw new ApiError(400, 'invalid_input', 'Invalid external_files')
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
	const encoded = JSON.stringify(manifest)
	const requestKey = request.headers.get('idempotency-key') ?? crypto.randomUUID()
	if (!/^[a-zA-Z0-9_-]{1,128}$/.test(requestKey))
		throw new ApiError(400, 'invalid_input', 'Invalid idempotency key')
	const requestHash = Array.from(
		new Uint8Array(
			await crypto.subtle.digest(
				'SHA-256',
				new TextEncoder().encode(
					JSON.stringify({
						manifest,
						external_files: external.map(({ file_name, file_type }) => ({ file_name, file_type })),
					}),
				),
			),
		),
		(byte) => byte.toString(16).padStart(2, '0'),
	).join('')
	// One D1 batch allocates the monotonic version and all file rows atomically.
	const statements = [
		env.DB.prepare(
			'INSERT OR IGNORE INTO shared_versions (instance_id,version,manifest,ready,created,request_key,request_hash) SELECT ?,COALESCE(MAX(version),0)+1,?,?,?,?,? FROM shared_versions WHERE instance_id = ?',
		).bind(
			item.id,
			encoded,
			external.length === 0 ? 1 : 0,
			now(),
			requestKey,
			requestHash,
			item.id,
		),
	]
	for (const file of external)
		statements.push(
			env.DB.prepare(
				'INSERT OR IGNORE INTO shared_files (id,instance_id,version,file_name,file_type) SELECT ?,?,version,?,? FROM shared_versions WHERE instance_id = ? AND request_key = ? AND request_hash = ?',
			).bind(file.id, item.id, file.file_name, file.file_type, item.id, requestKey, requestHash),
		)
	statements.push(
		env.DB.prepare('SELECT * FROM shared_versions WHERE instance_id = ? AND request_key = ?').bind(
			item.id,
			requestKey,
		),
	)
	const result = await env.DB.batch<Version>(statements)
	const version = result[result.length - 1]?.results[0]
	if (!version) throw new ApiError(500, 'database_error', 'Could not create shared version')
	if (version.request_hash !== requestHash)
		throw new ApiError(409, 'conflict', 'Idempotency key was already used for a different version')
	if (version.ready) {
		await notifyReadyInvites(env, item.id)
		await pruneVersions(env, item.id)
	}
	return json(await versionResponse(env, request, version, userId, true))
}

async function upload(
	request: Request,
	env: Env,
	token: string,
	operation?: string,
): Promise<Response> {
	const user = await requireUser(request, env)
	const file = await env.DB.prepare('SELECT * FROM shared_files WHERE id = ?')
		.bind(token)
		.first<File>()
	if (!file) throw new ApiError(404, 'not_found', 'Upload was not found')
	const item = await instance(env, file.instance_id)
	owner(item, user.id)
	let stored = file.blob_id ? await blob(env, file.blob_id) : null
	const identity = {
		instance_id: item.id,
		version: file.version,
		file_name: file.file_name,
		file_type: file.file_type,
	}
	if (operation === 'status' && request.method === 'GET') {
		const available = stored && (await verifiedReplica(env, stored, file.id))
		return json({
			...identity,
			status: available ? 'available' : 'pending',
			sha256: stored?.sha256 ?? null,
			size: stored?.size ?? null,
		})
	}
	if (operation === 'prepare' && request.method === 'POST') {
		const body = await readJson(request)
		const sha256 = validHash(body.sha256)
		const size = validSize(body.size)
		if (stored && (stored.sha256 !== sha256 || stored.size !== size))
			throw new ApiError(
				409,
				'integrity_error',
				'A version cannot be changed after preparing its upload',
			)
		if (!stored) {
			await env.DB.batch([
				env.DB.prepare(
					'INSERT OR IGNORE INTO shared_blobs (id,instance_id,sha256,size,created) VALUES (?,?,?,?,?)',
				).bind(crypto.randomUUID(), item.id, sha256, size, now()),
				env.DB.prepare(
					'UPDATE shared_files SET blob_id = (SELECT id FROM shared_blobs WHERE instance_id = ? AND sha256 = ?) WHERE id = ? AND blob_id IS NULL',
				).bind(item.id, sha256, file.id),
			])
			stored = await env.DB.prepare(
				'SELECT b.* FROM shared_blobs b JOIN shared_files f ON f.blob_id = b.id WHERE f.id = ?',
			)
				.bind(file.id)
				.first<StoredBlob>()
			if (!stored || stored.sha256 !== sha256 || stored.size !== size)
				throw new ApiError(409, 'integrity_error', 'Concurrent upload has different bytes')
		}
		const placement = await prepareUpload(env, new URL(request.url).origin, file.id, stored)
		if (placement.status === 'available') await finalizeVersion(env, file.id)
		return json({ ...identity, ...placement })
	}
	if (request.method !== 'PUT' || operation)
		throw new ApiError(405, 'method_not_allowed', 'Unsupported upload operation')
	const node = (await onlineNodes(env))[0]
	if (!node)
		throw new ApiError(
			503,
			'storage_unavailable',
			'No storage node is online; the owner snapshot can retry later',
		)
	const size = validSize(Number(request.headers.get('content-length')))
	if (stored && stored.size !== size)
		throw new ApiError(409, 'integrity_error', 'Upload size does not match the version')
	const destination = await uploadUrl(
		node,
		new URL(request.url).origin,
		file.id,
		stored?.sha256 ?? null,
		size,
	)
	const response = await fetch(destination, {
		method: 'PUT',
		body: request.body,
		headers: { 'content-length': String(size) },
	})
	if (!response.ok)
		throw new ApiError(
			response.status,
			'upload_failed',
			'Storage did not commit the complete upload',
		)
	await response.body?.cancel()
	await finalizeVersion(env, file.id)
	return empty()
}

async function download(request: Request, env: Env, token: string): Promise<Response> {
	const access = await env.DB.prepare(
		'SELECT file_id,user_id FROM shared_downloads WHERE token = ? AND expires > ?',
	)
		.bind(token, Date.now())
		.first<{ file_id: string; user_id: string }>()
	if (!access)
		throw new ApiError(401, 'unauthorized', 'Download authorization expired; retry the install')
	const file = await env.DB.prepare('SELECT * FROM shared_files WHERE id = ?')
		.bind(access.file_id)
		.first<File>()
	if (!file?.blob_id) throw new ApiError(404, 'not_found', 'Shared file was not found')
	const item = await instance(env, file.instance_id)
	await member(env, item, access.user_id)
	if (item.quarantine) throw new ApiError(403, 'forbidden', 'Shared instance is quarantined')
	return downloadBlob(request, env, await blob(env, file.blob_id), file.id)
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
		const id = crypto.randomUUID()
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
		![
			'instances',
			'invites',
			'uploads',
			'downloads',
			'blacklist',
			'moderation',
			'storage',
			'icons',
		].includes(parts[1] ?? '')
	)
		return null
	const [, route, id, section, child] = parts
	if (route === 'invites' && id && request.method === 'GET') return previewInvite(env, id)
	if (route === 'downloads' && id && ['GET', 'HEAD'].includes(request.method))
		return download(request, env, id)
	if (route === 'uploads' && id) return upload(request, env, id, section)
	if (route === 'storage' && request.method === 'POST') {
		if (id === 'receipt') {
			const receipt = await acceptStorageReceipt(request, env)
			await finalizeVersion(env, receipt.fileId)
			return empty()
		}
		if (id && section === 'heartbeat') return storageHeartbeat(request, env, id)
	}
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
			await pruneVersions(env, section)
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
			return json(await versionResponse(env, request, version, user.id))
		}
	}
	if (section === 'recovery' && request.method === 'GET') {
		owner(item, user.id)
		const rows = (
			await env.DB.prepare(
				'SELECT * FROM shared_files WHERE instance_id = ? AND blob_id IS NOT NULL ORDER BY version DESC',
			)
				.bind(id)
				.all<File>()
		).results
		const recovery = []
		for (const file of rows) {
			const stored = await blob(env, file.blob_id!)
			recovery.push({
				version: file.version,
				file_name: file.file_name,
				file_type: file.file_type,
				sha256: stored.sha256,
				file_size: stored.size,
				url: `${url.origin}/v1/uploads/${file.id}`,
				missing: !(await verifiedReplica(env, stored, file.id)),
			})
		}
		return json({ files: recovery })
	}
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
