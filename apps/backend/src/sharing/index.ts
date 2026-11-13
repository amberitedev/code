import {
	ALLOWANCE_BYTES,
	type CreatedId,
	type FollowedShare,
	type InstanceUser,
	type InstanceVersion,
	type Invite,
	type InviteLink,
	type InviteUser,
	MAX_SHARE_BYTES,
	type NewInstanceVersion,
	type NewInviteLink,
	type NewSharedInstance,
	type OutsideAddress,
	READ_TICKET_HEADER,
	type RemovedFile,
	type SharedIconConfig,
	type SharedInstanceUpdate,
	type UserIds,
	type VersionFault,
	type VersionManifest,
	sharing,
} from '@amberite/contracts'

import { requireUnlocked, requireUser, tokenFrom } from '../common/auth'
import { randomId } from '../common/crypto'
import { ApiError } from '../common/http'
import { notificationResponse, notifyMembers } from '../common/notifications'
import { JsonText, noBody, rawBody, route, serve, spliceJson } from '../common/router'
import type { Env } from '../common/types'
import { requireListedCore } from '../cores'
import { listHostedServers } from '../cores/servers'
import { readTicket, readTicketUser } from '../cores/tickets'
import { canInviteToSharedInstance } from '../social/preferences'
import { notifyUser } from '../social/socket'
import {
	EMPTY_ROOT,
	MAX_MANIFEST_BODY,
	MAX_VERSION_FILES,
	KEPT_VERSIONS,
	UNCONFIRMED_TTL_MS,
	addedBytes,
	confirmedHolders,
	externalFile,
	manifestKey,
	ownerShares,
	place,
	shareUsage,
	versionBytes,
	versionDownload,
	versionHash,
	versionObjects,
	versionUpload,
} from './pool'

// Sharing metadata, as Modrinth's App expects it. The files themselves live on Cores; see pool.ts.
// Every version lists all of the share's files, so dropping older ones loses nothing.

type InstanceRow = {
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
	/** The Core whose key writes this share, for a server-first share; see `coreRoutes`. */
	core_id: string | null
	server_id: string | null
}
type VersionRow = {
	instance_id: string
	version: number
	version_hash: string
	bytes: number
	added: number | null
	ready: number
	created: string
}
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
/** A Core server with players, and the share it follows. */
type HostedServerRow = {
	core_id: string
	server_id: string
	instance_id: string
	name: string
	/** Its game port; players join it at the Core's address (`joinAddress`). */
	port: number
	/** `ip:port` the Core's router granted it on the internet, last known; see `joinAddress`. */
	outside: string | null
}
const now = () => new Date().toISOString()
const quarantined = () => new ApiError(403, 'quarantined', 'Shared instance is quarantined')

function string(value: unknown, field: string, max = 256): string {
	if (typeof value !== 'string' || value.length > max || !value.trim())
		throw new ApiError(400, 'invalid_input', `Invalid ${field}`)
	return value
}

/**
 * A shared file's name is its path inside the instance, such as `config/foo.toml`, or a bare file
 * name for content files. Slashes separate; nothing may escape the instance folder.
 */
function fileName(value: unknown): string {
	const name = string(value, 'file_name', 1024)
	if (
		/[\\\u0000-\u001f]/.test(name) ||
		name.split('/').some((part) => part === '' || part === '.' || part === '..')
	)
		throw new ApiError(400, 'invalid_input', 'Invalid file path')
	return name
}

function fileType(value: unknown): string {
	const type = string(value, 'file_type', 64)
	if (!/^[a-z0-9_-]+$/.test(type)) throw new ApiError(400, 'invalid_input', 'Invalid file_type')
	return type
}

const REMOVED_PARENTS = ['mods', 'plugins', 'datapacks', 'shaderpacks', 'resourcepacks']
/** The App drops a version whose removed path names any other kind of file. */
const REMOVED_SUFFIXES = ['.jar', '.zip', '.jar.disabled', '.zip.disabled']
/** Windows device names, as `is_reserved_windows_device_name` in `packages/path-util`. */
const RESERVED_DEVICE_NAMES = [
	...['CON', 'PRN', 'AUX', 'NUL', 'CONIN$', 'CONOUT$'],
	...['COM', 'LPT'].flatMap((name) => [...'123456789¹²³'].map((digit) => name + digit)),
]

/**
 * Whether `name` is one plain file name, as `path_util::is_safe_file_name` decides: no
 * separator of either kind, no drive prefix, not `.` or `..`, and no Windows device name
 * (`CON`, `con.jar`, `NUL:x`).
 */
function safeFileName(name: string): boolean {
	if (!name || name === '.' || name === '..' || /[/\\]/.test(name) || /^[a-z]:/i.test(name))
		return false
	const upper = name.replace(/[a-z]/g, (letter) => letter.toUpperCase())
	return !RESERVED_DEVICE_NAMES.some(
		(device) => upper.startsWith(device) && /^($|[.:])/.test(upper.slice(device.length)),
	)
}

/** `removed_files` by the App's own rules (`SharedInstanceRemovedFile::validate` in app-lib). */
function removedFiles(value: unknown, modpackId: string | null): RemovedFile[] {
	if (value === undefined || value === null) return []
	if (!Array.isArray(value) || value.length > MAX_VERSION_FILES)
		throw new ApiError(400, 'invalid_input', 'Invalid removed_files')
	if (value.length && !modpackId)
		throw new ApiError(400, 'invalid_input', 'Removed modpack files require a modpack_id')
	return value.map((item): RemovedFile => {
		const entry = object(item)
		if (entry.type === 'version')
			return { type: 'version', version_id: string(entry.version_id, 'version_id') }
		const filename = typeof entry.filename === 'string' ? entry.filename : ''
		if (
			entry.type !== 'path' ||
			typeof entry.parent !== 'string' ||
			!REMOVED_PARENTS.includes(entry.parent) ||
			/[\u0000-\u001f]/.test(filename) ||
			!safeFileName(filename) ||
			!REMOVED_SUFFIXES.some((suffix) => filename.endsWith(suffix))
		)
			throw new ApiError(400, 'invalid_input', 'Invalid removed modpack file')
		return { type: 'path', parent: entry.parent, filename }
	})
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

async function instance(env: Env, id: string): Promise<InstanceRow> {
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
const LIVE_SERVER = `EXISTS (SELECT 1 FROM cores lc JOIN shared_instances li ON li.id = s.instance_id WHERE lc.id = s.core_id
	AND (li.owner_id = lc.owner_id OR EXISTS (SELECT 1 FROM shared_instance_users lm WHERE lm.instance_id = li.id AND lm.user_id = lc.owner_id AND lm.joined_at IS NOT NULL)))`

/**
 * SQL joining a server's players (`p`) to the server (`s`), which names the share it follows.
 * Only servers that may pass it on (`LIVE_SERVER`), so everything players read goes through it.
 */
const PLAYERS = `hosted_server_players p JOIN hosted_servers s ON s.core_id = p.core_id AND s.server_id = p.server_id AND ${LIVE_SERVER}`

/** Whether `userId` owns the share or has joined it as a member. Players are not members. */
async function joinedMember(env: Env, item: InstanceRow, userId: string): Promise<boolean> {
	if (item.owner_id === userId) return true
	return !!(await env.DB.prepare(
		'SELECT 1 FROM shared_instance_users WHERE instance_id = ? AND user_id = ? AND joined_at IS NOT NULL',
	)
		.bind(item.id, userId)
		.first())
}

/**
 * Delete the servers of `ownerIds`' Cores that follow the share, with their players and links:
 * they lost the share, so their servers cannot pass it on. Statements for the caller's batch.
 */
function dropServers(env: Env, instanceId: string, ownerIds: string[]): D1PreparedStatement[] {
	return ownerIds.flatMap((ownerId) => [
		env.DB.prepare(
			'DELETE FROM shared_instance_invites WHERE instance_id = ? AND server_id IS NOT NULL AND core_id IN (SELECT id FROM cores WHERE owner_id = ?)',
		).bind(instanceId, ownerId),
		env.DB.prepare(
			'DELETE FROM hosted_servers WHERE instance_id = ? AND core_id IN (SELECT id FROM cores WHERE owner_id = ?)',
		).bind(instanceId, ownerId),
	])
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
 * Whoever reads the share: its owner, its members, and the players of every server that follows
 * it. Pending ones too with `allowPending`, so an invite can be previewed.
 */
async function member(
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

function owner(item: InstanceRow, userId: string) {
	if (item.owner_id !== userId)
		throw new ApiError(403, 'not_owner', 'Only the owner can change this shared instance')
}

async function requireSharing(env: Env, userId: string): Promise<void> {
	if (
		await env.DB.prepare('SELECT 1 FROM sharing_blacklist WHERE user_id = ?').bind(userId).first()
	)
		throw new ApiError(403, 'blacklisted', 'This account cannot use sharing')
}

/** The manifest of a version, read from R2 as text so the Worker never parses it. */
async function manifestText(env: Env, item: Pick<VersionRow, 'instance_id' | 'version'>) {
	const object = await env.FILES.get(manifestKey(item.instance_id, item.version))
	if (!object) throw new ApiError(404, 'version_not_found', 'Shared version was not found')
	return new JsonText<VersionManifest>(await object.text())
}

/**
 * A version as the App reads it: the stored manifest, spliced as text, plus what the pool adds.
 * A member of a ready version gets the confirmed holders and a fetch ticket; the owner of an
 * unconfirmed one gets the home and a store ticket, so a crashed push resumes; the owner also
 * sees how many members reported broken files. A Core's read (`userId` null) gets no owner
 * extras: its push takes the store ticket from creating the version.
 */
async function versionResponse(
	env: Env,
	item: InstanceRow,
	version: VersionRow,
	userId: string | null,
): Promise<JsonText<InstanceVersion>> {
	const manifest = await manifestText(env, version)
	const extra: Omit<InstanceVersion, keyof VersionManifest> = {
		version: version.version,
		ready: version.ready === 1,
	}
	if (version.version_hash !== EMPTY_ROOT) {
		if (version.ready) {
			const holders = await confirmedHolders(env, [item.home, item.replica], version.version_hash)
			extra.download = await versionDownload(env, holders, version.version_hash)
		} else if (item.owner_id === userId && item.home) {
			extra.upload = await versionUpload(env, item.home, version.version_hash, version.bytes)
		}
	}
	if (item.owner_id === userId) {
		const reports = await env.DB.prepare(
			"SELECT COUNT(*) AS content FROM shared_instance_reports WHERE instance_id = ? AND version = ? AND kind = 'content'",
		)
			.bind(item.id, version.version)
			.first<{ content: number }>()
		extra.reports = { content: reports?.content ?? 0 }
	}
	return spliceJson(manifest, extra)
}

/** The newest complete version, or the one being uploaded when none is complete yet. */
async function latestVersion(env: Env, id: string): Promise<VersionRow> {
	const result = await env.DB.prepare(
		'SELECT * FROM shared_instance_versions WHERE instance_id = ? ORDER BY ready DESC, version DESC LIMIT 1',
	)
		.bind(id)
		.first<VersionRow>()
	if (!result) throw new ApiError(404, 'version_not_found', 'Shared instance has no version')
	return result
}

/**
 * Deletes the instance's manifests that no remaining version row uses. Called after the rows are
 * gone, so it clears everything once the instance itself is deleted. The Cores holding the files
 * learn what to drop at their next storage report.
 */
export async function sweepFiles(env: Env, id: string): Promise<void> {
	const stored: string[] = []
	let cursor: string | undefined
	do {
		const page = await env.FILES.list({ prefix: `manifests/${id}/`, cursor })
		stored.push(...page.objects.map((object) => object.key))
		cursor = page.truncated ? page.cursor : undefined
	} while (cursor)
	const used = new Set(
		(
			await env.DB.prepare('SELECT version FROM shared_instance_versions WHERE instance_id = ?')
				.bind(id)
				.all<{ version: number }>()
		).results.map((row) => manifestKey(id, row.version)),
	)
	const unused = stored.filter((key) => !used.has(key))
	if (unused.length) await env.FILES.delete(unused)
	const icons = await env.FILES.list({ prefix: `icons/${id}/` })
	if (icons.objects.length) await env.FILES.delete(icons.objects.map((object) => object.key))
}

/**
 * Keeps the latest `KEPT_VERSIONS` complete versions and drops uploads abandoned before the
 * newest one, with their manifests. Deletes by name, so a push does not pay for listing the
 * instance's manifests. The Cores holding the files drop them at their next storage report.
 */
async function pruneVersions(env: Env, id: string): Promise<void> {
	const { results } = await env.DB.prepare(
		`DELETE FROM shared_instance_versions WHERE instance_id = ?1 AND (
			version < (SELECT MIN(version) FROM (SELECT version FROM shared_instance_versions WHERE instance_id = ?1 AND ready = 1 ORDER BY version DESC LIMIT ?2))
			OR (ready = 0 AND version < (SELECT MAX(version) FROM shared_instance_versions WHERE instance_id = ?1 AND ready = 1)))
		RETURNING version`,
	)
		.bind(id, KEPT_VERSIONS)
		.all<{ version: number }>()
	if (results.length) await env.FILES.delete(results.map((row) => manifestKey(id, row.version)))
}

async function notifyReadyInvites(env: Env, id: string): Promise<void> {
	const ready = await env.DB.prepare(
		'SELECT 1 FROM shared_instance_versions WHERE instance_id = ? AND ready = 1 LIMIT 1',
	)
		.bind(id)
		.first()
	if (!ready) return
	const item = await instance(env, id)
	const members = (
		await env.DB.prepare(
			'SELECT user_id FROM shared_instance_users WHERE instance_id = ? AND joined_at IS NULL AND notified = 0',
		)
			.bind(id)
			.all<{ user_id: string }>()
	).results
	// A player is invited to the server, by its Core's owner, under the server's name.
	const players = (
		await env.DB.prepare(
			`SELECT p.user_id, p.core_id, p.server_id, s.name, c.owner_id FROM ${PLAYERS} JOIN cores c ON c.id = p.core_id WHERE s.instance_id = ? AND p.joined_at IS NULL AND p.notified = 0`,
		)
			.bind(id)
			.all<{
				user_id: string
				core_id: string
				server_id: string
				name: string
				owner_id: string
			}>()
	).results
	// `rows` selects the invite's own pending row, so a concurrent call sends it only once.
	const invites = [
		...members.map((row) => ({
			user: row.user_id,
			name: item.name,
			by: item.owner_id,
			table: 'shared_instance_users',
			rows: 'instance_id = ?1 AND user_id = ?2 AND joined_at IS NULL',
			keys: [id, row.user_id],
		})),
		...players.map((row) => ({
			user: row.user_id,
			name: row.name,
			by: row.owner_id,
			table: 'hosted_server_players',
			rows: 'core_id = ?1 AND server_id = ?2 AND user_id = ?3 AND joined_at IS NULL',
			keys: [row.core_id, row.server_id, row.user_id],
		})),
	]
	for (const invite of invites) {
		const notification = {
			id: randomId(),
			user_id: invite.user,
			body: JSON.stringify({
				type: 'shared_instance_invite',
				shared_instance_id: id,
				shared_instance_name: invite.name,
				shared_instance_icon: item.icon,
				shared_instance_icon_config: iconConfig(item),
				invited_by: invite.by,
			}),
			created: now(),
			read: 0,
		}
		const at = invite.keys.length
		const result = await env.DB.batch([
			env.DB.prepare(
				`INSERT INTO notifications (id,user_id,body,created) SELECT ?${at + 1},?${at + 2},?${at + 3},?${at + 4} WHERE EXISTS (SELECT 1 FROM ${invite.table} WHERE ${invite.rows} AND notified = 0)`,
			).bind(
				...invite.keys,
				notification.id,
				notification.user_id,
				notification.body,
				notification.created,
			),
			env.DB.prepare(`UPDATE ${invite.table} SET notified = 1 WHERE ${invite.rows}`).bind(
				...invite.keys,
			),
		])
		if (result[0]?.meta.changes)
			await notifyUser(env, invite.user, notificationResponse(notification)).catch(() => undefined)
	}
}

/**
 * A version became ready: its home Core confirmed it holds the files. Invites waiting on
 * something to install go out, and older versions are pruned.
 */
export async function versionReady(env: Env, id: string, version: number): Promise<void> {
	const result = await env.DB.prepare(
		'UPDATE shared_instance_versions SET ready = 1 WHERE instance_id = ? AND version = ? AND ready = 0',
	)
		.bind(id, version)
		.run()
	if (!result.meta.changes) return
	await notifyReadyInvites(env, id)
	await pruneVersions(env, id)
}

/**
 * The owner's App pushed: it sends the manifest with every file already encrypted and named, and
 * gets back the home Core and a store ticket to upload to. The version is ready once that Core
 * confirms. A push with the same files as an unconfirmed version resumes it; one with other files
 * waits until that version is confirmed or an hour old. `server_core` names the owner's Core
 * running a server of this instance, where the share is then homed (see `place`). A Core pushing
 * its own server's share passes itself as `core`, which is that server's Core.
 */
/** A push's body, checked as far as it can be without hashing. */
function parseNewVersion(raw: unknown): NewInstanceVersion {
	const body = object(raw)
	const loader = string(body.loader, 'loader')
	if (!['vanilla', 'fabric', 'forge', 'quilt', 'neoforge'].includes(loader))
		throw new ApiError(400, 'invalid_input', 'Invalid loader')
	if (!Array.isArray(body.external_files))
		throw new ApiError(400, 'invalid_input', 'Invalid external_files')
	if (body.external_files.length > MAX_VERSION_FILES)
		throw new ApiError(
			413,
			'too_many_files',
			`A shared instance can hold at most ${MAX_VERSION_FILES} files`,
		)
	const seen = new Set<string>()
	const external = body.external_files.map((value) => {
		const entry = object(value)
		const file_name = fileName(entry.file_name)
		const file_type = fileType(entry.file_type)
		const name = `${file_type}:${file_name}`
		if (seen.has(name)) throw new ApiError(400, 'invalid_input', 'Duplicate external file')
		seen.add(name)
		return externalFile(entry, file_name, file_type)
	})
	const modpackId =
		body.modpack_id === undefined || body.modpack_id === null
			? null
			: string(body.modpack_id, 'modpack_id')
	return {
		modrinth_ids: strings(body.modrinth_ids ?? [], 'modrinth_ids', 1000),
		modpack_id: modpackId,
		removed_files: removedFiles(body.removed_files, modpackId),
		game_version: string(body.game_version, 'game_version'),
		loader,
		loader_version: typeof body.loader_version === 'string' ? body.loader_version : '',
		version_hash: typeof body.version_hash === 'string' ? body.version_hash : '',
		external_files: external,
		server_core:
			body.server_core === undefined || body.server_core === null
				? undefined
				: string(body.server_core, 'server_core', 64),
	}
}

async function createVersion(
	env: Env,
	item: InstanceRow,
	userId: string,
	{ server_core, ...body }: NewInstanceVersion,
	core: string | null = null,
): Promise<JsonText<InstanceVersion>> {
	const external = body.external_files
	const manifest: VersionManifest = {
		...body,
		version_hash: await versionHash(external.map((file) => file.object)),
	}
	if (external.length && body.version_hash !== manifest.version_hash)
		throw new ApiError(400, 'invalid_input', 'version_hash does not match the files')
	const bytes = versionBytes(external)
	if (bytes > MAX_SHARE_BYTES)
		throw new ApiError(413, 'share_too_large', 'A shared instance can hold at most 1 GB of files')
	const serverCore = core ?? server_core ?? null

	// One push at a time: an unconfirmed version younger than its store ticket is still being
	// uploaded, unless this push is the same version again, which resumes it.
	const pending = await env.DB.prepare(
		'SELECT * FROM shared_instance_versions WHERE instance_id = ? AND ready = 0 ORDER BY version DESC LIMIT 1',
	)
		.bind(item.id)
		.first<VersionRow>()
	if (pending) {
		const fresh = Date.parse(pending.created) > Date.now() - UNCONFIRMED_TTL_MS
		if (fresh && pending.version_hash === manifest.version_hash) {
			await env.FILES.put(manifestKey(item.id, pending.version), JSON.stringify(manifest))
			return versionResponse(env, item, pending, userId)
		}
		if (fresh)
			throw new ApiError(
				409,
				'push_in_flight',
				'Another push of this instance is still uploading. Try again when it finishes.',
			)
		await env.DB.prepare(
			'DELETE FROM shared_instance_versions WHERE instance_id = ? AND version = ?',
		)
			.bind(item.id, pending.version)
			.run()
	}

	// Where the files go, and what they add to the owner's usage: the share's kept versions once
	// this one is ready and those beyond the owner's setting are dropped.
	let added = bytes
	if (bytes) {
		const members = await env.DB.prepare(
			`SELECT 1 FROM shared_instance_users WHERE instance_id = ?1 AND join_type != 'owner' UNION ALL SELECT 1 FROM ${PLAYERS} WHERE s.instance_id = ?1 LIMIT 1`,
		)
			.bind(item.id)
			.first()
		const placement = await place(
			env,
			userId,
			{ home: item.home, replica: item.replica },
			{ serverCore, members: members !== null },
		)
		if (!placement.home)
			throw new ApiError(
				503,
				'no_storage',
				'No storage is available for shared files right now. Try again later.',
			)
		const ready = (
			await env.DB.prepare(
				'SELECT version, bytes, added FROM shared_instance_versions WHERE instance_id = ? AND ready = 1 ORDER BY version',
			)
				.bind(item.id)
				.all<Pick<VersionRow, 'version' | 'bytes' | 'added'>>()
		).results
		const previous = ready.at(-1)
		if (previous)
			added = addedBytes(
				new Map(external.map((file) => [file.object, file.object_size])),
				await versionObjects(env, item.id, previous.version),
			)
		const [shares, own] = await Promise.all([
			ownerShares(env, userId),
			env.DB.prepare('SELECT id FROM cores WHERE owner_id = ?').bind(userId).all<{ id: string }>(),
		])
		const counted = [placement.home, placement.replica].some(
			(id) => id !== null && !own.results.some((core) => core.id === id),
		)
		if (counted) {
			const after = [...ready, { bytes, added }]
			const others = shares
				.filter((share) => share.id !== item.id)
				.reduce((sum, share) => sum + share.used, 0)
			if (others + shareUsage(after.slice(-KEPT_VERSIONS)) > ALLOWANCE_BYTES)
				throw new ApiError(
					413,
					'storage_quota',
					`This push needs more than your ${ALLOWANCE_BYTES / 1024 ** 3} GB of shared storage. Delete old versions to free space.`,
				)
		}
		if (placement.home !== item.home || placement.replica !== item.replica) {
			await env.DB.prepare('UPDATE shared_instances SET home = ?, replica = ? WHERE id = ?')
				.bind(placement.home, placement.replica, item.id)
				.run()
			item = { ...item, ...placement }
		}
	}
	const home = item.home
	// The number is allocated in the insert itself, so two pushes at once never share one, and
	// the insert refuses while another version is still uploading, so two pushes cannot both get
	// a store ticket. The row starts unready, so nobody reads it before its manifest is in R2; a
	// failed write takes the row back.
	const version = await env.DB.prepare(
		'INSERT INTO shared_instance_versions (instance_id,version,version_hash,bytes,added,ready,created) SELECT ?1,COALESCE(m,0)+1,?2,?3,?6,0,?4 FROM (SELECT MAX(version) AS m FROM shared_instance_versions WHERE instance_id = ?1) WHERE NOT EXISTS (SELECT 1 FROM shared_instance_versions WHERE instance_id = ?1 AND ready = 0 AND created > ?5) RETURNING *',
	)
		.bind(
			item.id,
			manifest.version_hash,
			bytes,
			now(),
			new Date(Date.now() - UNCONFIRMED_TTL_MS).toISOString(),
			added,
		)
		.first<VersionRow>()
	if (!version)
		throw new ApiError(
			409,
			'push_in_flight',
			'Another push of this instance is still uploading. Try again when it finishes.',
		)
	try {
		await env.FILES.put(manifestKey(item.id, version.version), JSON.stringify(manifest))
	} catch (error) {
		await env.DB.prepare(
			'DELETE FROM shared_instance_versions WHERE instance_id = ? AND version = ?',
		)
			.bind(item.id, version.version)
			.run()
		throw error
	}
	// Ready at once with no files, or when the home already holds these exact objects.
	const held =
		bytes && home
			? await env.DB.prepare('SELECT 1 FROM core_holdings WHERE core_id = ? AND version_hash = ?')
					.bind(home, manifest.version_hash)
					.first()
			: null
	if (!bytes || held) {
		await versionReady(env, item.id, version.version)
		version.ready = 1
	}
	return versionResponse(env, item, version, userId)
}

/**
 * A member says a version's bytes were wrong. `object`: a Core served bytes that did not hash to
 * their name. `content`: they decrypted to the wrong file, which only a new push fixes. The report
 * stays with the reporter and is shown to the owner; nothing is marked broken for everyone.
 */
async function reportVersion(
	env: Env,
	item: InstanceRow,
	version: VersionRow,
	userId: string,
	fault: VersionFault,
): Promise<void> {
	await env.DB.prepare(
		'INSERT OR REPLACE INTO shared_instance_reports (instance_id,version,user_id,core_id,kind,created) VALUES (?,?,?,?,?,?)',
	)
		.bind(item.id, version.version, userId, fault.core_id, fault.kind, now())
		.run()
}

function parseFault(raw: unknown): VersionFault {
	const body = object(raw)
	const kind = string(body.kind, 'kind', 16)
	if (kind !== 'object' && kind !== 'content')
		throw new ApiError(400, 'invalid_input', 'Invalid kind')
	return { core_id: string(body.core_id, 'core_id', 64), kind }
}

/**
 * The owner deletes an older version to free space. The current version, which members install,
 * stays, and so does one still uploading. The next version's added bytes are counted again
 * against the version before the deleted one, so usage stays right.
 */
async function deleteVersion(env: Env, item: InstanceRow, version: VersionRow): Promise<void> {
	const current = await latestVersion(env, item.id)
	if (!version.ready || current.version === version.version)
		throw new ApiError(
			409,
			'current_version',
			version.ready ? 'The current version cannot be deleted.' : 'This version is still uploading.',
		)
	const neighbour = (direction: '<' | '>') =>
		env.DB.prepare(
			`SELECT version FROM shared_instance_versions WHERE instance_id = ? AND ready = 1 AND version ${direction} ? ORDER BY version ${direction === '<' ? 'DESC' : 'ASC'} LIMIT 1`,
		)
			.bind(item.id, version.version)
			.first<{ version: number }>()
	const [previous, next] = await Promise.all([neighbour('<'), neighbour('>')])
	const remove = env.DB.prepare(
		'DELETE FROM shared_instance_versions WHERE instance_id = ? AND version = ?',
	).bind(item.id, version.version)
	if (previous && next) {
		const [after, before] = await Promise.all([
			versionObjects(env, item.id, next.version),
			versionObjects(env, item.id, previous.version),
		])
		await env.DB.batch([
			remove,
			env.DB.prepare(
				'UPDATE shared_instance_versions SET added = ? WHERE instance_id = ? AND version = ?',
			).bind(addedBytes(after, before), item.id, next.version),
		])
	} else await remove.run()
	await env.FILES.delete(manifestKey(item.id, version.version))
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

/** Seconds a link lives, a week by default; 0 is a year. Uses, 0 for no limit. */
function parseInviteLink(body: Record<string, unknown>): NewInviteLink {
	return {
		max_age: natural(body.max_age, 604800, 31536000),
		max_uses: natural(body.max_uses, 0, 10000),
	}
}

/** A new invite link to the share `instanceId`, or with `server`, to be a player of that server. */
async function createLink(
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

function parseUserIds(raw: unknown): UserIds {
	return { user_ids: strings(object(raw).user_ids, 'user_ids', 100) }
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

/** The server a players write names: the share it follows, its name, its game port, and `net.outside`. */
function parseFollowedShare(body: Record<string, unknown>): FollowedShare {
	return {
		instance_id: string(body.instance_id, 'instance_id', 64),
		name: string(body.name, 'name'),
		port: gamePort(body.port),
		outside: outsideAddress(body.outside),
	}
}

function gamePort(value: unknown): number {
	if (typeof value !== 'number' || !Number.isInteger(value) || value < 1 || value > 65535)
		throw new ApiError(400, 'invalid_input', 'Invalid port')
	return value
}

/** `net.outside`; null while the Core has none. */
function outsideAddress(value: unknown): OutsideAddress | null {
	if (value === undefined || value === null) return null
	const outside = object(value)
	const ip = string(outside.ip, 'outside', 45)
	if (!/^[\da-f.:]+$/i.test(ip)) throw new ApiError(400, 'invalid_input', 'Invalid outside')
	return { ip, port: gamePort(outside.port) }
}

/**
 * Where players join a server: the `ip:port` its Core's router granted it, when known; otherwise
 * the host of the Core's last reported address, the one reached from outside when it has one,
 * with the server's port. Read each time, so it follows the Core's reports.
 */
function joinAddress(
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
async function cameThrough(env: Env, item: InstanceRow, userId: string) {
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

/** The signed-in caller, refused while sharing is closed to them. */
async function sharer(request: Request, env: Env) {
	const user = await requireUser(request, env)
	await requireSharing(env, user.id)
	return user
}

/** A version number from the path, which arrives as text. */
function versionNumber(value: number): number {
	const text = String(value)
	if (!/^\d+$/.test(text) || !Number.isSafeInteger(Number(text)))
		throw new ApiError(400, 'invalid_input', 'Invalid shared version')
	return Number(text)
}

async function versionRow(env: Env, id: string, version: number): Promise<VersionRow> {
	const row = await env.DB.prepare(
		'SELECT * FROM shared_instance_versions WHERE instance_id = ? AND version = ?',
	)
		.bind(id, versionNumber(version))
		.first<VersionRow>()
	if (!row) throw new ApiError(404, 'version_not_found', 'Shared version was not found')
	return row
}

/**
 * A version a member reads: the latest without `version`. Members only see versions once they are
 * ready; the App reads the 404 as "never ready".
 */
async function memberVersion(
	env: Env,
	item: InstanceRow,
	userId: string,
	version?: number,
): Promise<VersionRow> {
	if (item.quarantine) throw quarantined()
	if (version === undefined) return latestVersion(env, item.id)
	const row = await versionRow(env, item.id, version)
	if (!row.ready && item.owner_id !== userId)
		throw new ApiError(404, 'version_not_found', 'Shared version was not found')
	return row
}

/** A share the caller reads, pending invitees too with `allowPending`. */
async function readable(env: Env, id: string, userId: string, allowPending = false) {
	const item = await instance(env, id)
	await member(env, item, userId, allowPending)
	return item
}

async function owned(env: Env, id: string, userId: string) {
	const item = await readable(env, id, userId, true)
	owner(item, userId)
	return item
}

async function deleteShare(env: Env, id: string): Promise<void> {
	await env.DB.prepare('DELETE FROM shared_instances WHERE id = ?').bind(id).run()
	await sweepFiles(env, id)
}

async function renameShare(env: Env, id: string, update: SharedInstanceUpdate): Promise<void> {
	await env.DB.prepare('UPDATE shared_instances SET name = ? WHERE id = ?')
		.bind(update.name, id)
		.run()
}

const parseUpdate = (raw: unknown): SharedInstanceUpdate => ({
	name: string(object(raw).name, 'name'),
})

const parseNewShare = (raw: unknown): NewSharedInstance => {
	const body = object(raw)
	return {
		name: string(body.name, 'name'),
		server_id: body.server_id === undefined ? undefined : string(body.server_id, 'server_id', 64),
	}
}

const ICON_BYTES = 256 * 1024

/** Anyone may call these, signed in or not. */
const publicRoutes = [
	route(sharing.GetInvite, noBody, async ({ env, params, request }) =>
		previewInvite(env, params.id, new URL(request.url).searchParams.get('version') === 'true'),
	),
	route(sharing.GetIcon, noBody, async ({ env, params }) => {
		const object = await env.FILES.get(iconKey(params.id, params.upload))
		if (!object) throw new ApiError(404, 'icon_not_found', 'Icon was not found')
		return new Response(object.body, {
			headers: {
				'Content-Type': object.httpMetadata?.contentType ?? 'image/png',
				// Every upload gets a new URL, so a fetched icon never changes. Public, because
				// invite previews show it before anyone signs in.
				'Cache-Control': 'public, max-age=31536000, immutable',
			},
		})
	}),
]

/**
 * A Core, with its key, writing the share of one of its server-first servers (a server that
 * follows no instance). It acts as the share's owner, its own owner account, for: create (with
 * `server_id`, answering the server's existing share if it has one), rename, delete, and its
 * versions. A Core also reads the versions of a share one of its servers follows, for a user who
 * can read it and gave it a read ticket (`content::shared` in Core). It reaches no other share and
 * no other route.
 */
async function sharingCore(request: Request, env: Env, alwaysAllowed = false) {
	const core = await requireListedCore(request, env)
	// An owner may always delete their share, locked or not.
	if (!alwaysAllowed) {
		await requireUnlocked(env, core.owner_id)
		await requireSharing(env, core.owner_id)
	}
	return core
}

const notTheCoresShare = () =>
	new ApiError(403, 'not_owner', 'A Core reaches only the shares of its own servers')

/**
 * The share `id` of the Core's own server. The owner check matters because a Core picks its own
 * id when it pairs: a share left from an earlier pairing of that id is not this Core's.
 */
async function coreShare(env: Env, core: { id: string; owner_id: string }, id: string) {
	const item = await instance(env, id)
	if (item.core_id !== core.id || item.owner_id !== core.owner_id) throw notTheCoresShare()
	return item
}

/**
 * A version a Core reads: of its own server's share, or of a share a server follows, for the user
 * whose read ticket it sends. Without one it reads as its own owner, as for its CLI. Either way
 * that user must still read the share, so a Core never reads a share for someone who did not ask
 * it to.
 */
async function coreVersion(request: Request, env: Env, id: string, version?: number) {
	const core = await sharingCore(request, env)
	const item = await instance(env, id)
	if (item.core_id !== core.id || item.owner_id !== core.owner_id) {
		const ticket = request.headers.get(READ_TICKET_HEADER)
		const reader = ticket ? await readTicketUser(env, ticket, core.id, id) : core.owner_id
		if (!reader)
			throw new ApiError(403, 'ticket_invalid', 'The read ticket is not for this Core and share')
		await requireUnlocked(env, reader)
		await requireSharing(env, reader)
		await member(env, item, reader)
	}
	if (item.quarantine) throw quarantined()
	const row =
		version === undefined ? await latestVersion(env, id) : await versionRow(env, id, version)
	return versionResponse(env, item, row, null)
}

const coreKeyRoutes = [
	route(sharing.CreateSharedInstance, parseNewShare, async ({ request, env, body }) => {
		const core = await sharingCore(request, env)
		const { name, server_id } = await body()
		if (!server_id) throw notTheCoresShare()
		await env.DB.prepare(
			'INSERT INTO shared_instances (id,owner_id,name,created,core_id,server_id) VALUES (?,?,?,?,?,?) ON CONFLICT DO NOTHING',
		)
			.bind(crypto.randomUUID(), core.owner_id, name, now(), core.id, server_id)
			.run()
		const row = await env.DB.prepare(
			'SELECT id FROM shared_instances WHERE core_id = ? AND server_id = ? AND owner_id = ?',
		)
			.bind(core.id, server_id, core.owner_id)
			.first<{ id: string }>()
		if (!row) throw notTheCoresShare()
		return { id: row.id }
	}),
	route(sharing.UpdateSharedInstance, parseUpdate, async ({ request, env, params, body }) => {
		await coreShare(env, await sharingCore(request, env), params.id)
		await renameShare(env, params.id, await body())
	}),
	route(sharing.DeleteSharedInstance, noBody, async ({ request, env, params }) => {
		await coreShare(env, await sharingCore(request, env, true), params.id)
		await deleteShare(env, params.id)
	}),
	route(
		sharing.CreateVersion,
		parseNewVersion,
		async ({ request, env, params, body }) => {
			const core = await sharingCore(request, env)
			const item = await coreShare(env, core, params.id)
			if (item.quarantine) throw quarantined()
			return createVersion(env, item, core.owner_id, await body(), core.id)
		},
		MAX_MANIFEST_BODY,
	),
	route(sharing.GetLatestVersion, noBody, async ({ request, env, params }) =>
		coreVersion(request, env, params.id),
	),
	route(sharing.GetVersion, noBody, async ({ request, env, params }) =>
		coreVersion(request, env, params.id, params.version),
	),
]

/** Pending routes come before the `{invite_id}` ones they would otherwise match. */
const accountRoutes = [
	route(sharing.ListSharedInstances, noBody, async ({ request, env }) => {
		const user = await sharer(request, env)
		const query = new URL(request.url).searchParams
		if (query.has('user') && query.get('user') !== user.id)
			throw new ApiError(403, 'not_own_list', 'Private instance lists belong to their account')
		// The caller's own shares, and with `include_joined` every share they can play too.
		const sql =
			query.get('include_joined') === 'true'
				? `SELECT instance_id FROM shared_instance_users WHERE user_id = ?1 AND joined_at IS NOT NULL UNION SELECT s.instance_id FROM ${PLAYERS} WHERE p.user_id = ?1 AND p.joined_at IS NOT NULL`
				: 'SELECT id AS instance_id FROM shared_instances WHERE owner_id = ?1'
		return (await env.DB.prepare(sql).bind(user.id).all<{ instance_id: string }>()).results.map(
			(row) => row.instance_id,
		)
	}),
	route(sharing.CreateSharedInstance, parseNewShare, async ({ request, env, body }) => {
		const user = await sharer(request, env)
		const { name } = await body()
		const instanceId = crypto.randomUUID()
		await env.DB.batch([
			env.DB.prepare(
				'INSERT INTO shared_instances (id,owner_id,name,created) VALUES (?,?,?,?)',
			).bind(instanceId, user.id, name, now()),
			env.DB.prepare(
				"INSERT INTO shared_instance_users (instance_id,user_id,joined_at,join_type) VALUES (?,?,?,'owner')",
			).bind(instanceId, user.id, now()),
		])
		return { id: instanceId }
	}),
	route(sharing.GetSharedInstance, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		const item = await readable(env, params.id, user.id, true)
		const server = await cameThrough(env, item, user.id)
		return {
			name: item.name,
			icon: item.icon,
			icon_config: iconConfig(item),
			quarantine: item.quarantine === 1,
			installed_once: item.installed_once === 1,
			// The App adds it to the instance's server list.
			linked_server: server
				? { domain: joinAddress(server), region: '', online_status: null }
				: null,
		}
	}),
	route(sharing.UpdateSharedInstance, parseUpdate, async ({ request, env, params, body }) => {
		const user = await sharer(request, env)
		await owned(env, params.id, user.id)
		await renameShare(env, params.id, await body())
	}),
	route(sharing.DeleteSharedInstance, noBody, async ({ request, env, params }) => {
		// An owner may always delete their share, locked or not.
		const user = await requireUser(request, env)
		await owned(env, params.id, user.id)
		await deleteShare(env, params.id)
	}),
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
		const failed: string[] = []
		for (const id of (await body()).user_ids) {
			if (id === user.id) continue
			if (!(await canInviteToSharedInstance(env, user.id, id))) {
				failed.push(id)
				continue
			}
			await env.DB.prepare(
				"INSERT OR IGNORE INTO shared_instance_users (instance_id,user_id,join_type) VALUES (?,?,'invite')",
			)
				.bind(item.id, id)
				.run()
		}
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
			...ids.map((id) =>
				env.DB.prepare(
					'DELETE FROM shared_instance_users WHERE instance_id = ? AND user_id = ?',
				).bind(item.id, id),
			),
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
	route(sharing.GetLatestVersion, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		const item = await readable(env, params.id, user.id)
		return versionResponse(env, item, await memberVersion(env, item, user.id), user.id)
	}),
	route(sharing.GetVersion, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		const item = await readable(env, params.id, user.id)
		const version = await memberVersion(env, item, user.id, params.version)
		return versionResponse(env, item, version, user.id)
	}),
	route(
		sharing.CreateVersion,
		parseNewVersion,
		async ({ request, env, params, body }) => {
			const user = await sharer(request, env)
			const item = await readable(env, params.id, user.id)
			if (item.quarantine) throw quarantined()
			owner(item, user.id)
			return createVersion(env, item, user.id, await body())
		},
		MAX_MANIFEST_BODY,
	),
	route(sharing.DeleteVersion, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		const item = await readable(env, params.id, user.id)
		const version = await memberVersion(env, item, user.id, params.version)
		owner(item, user.id)
		await deleteVersion(env, item, version)
	}),
	route(sharing.ReportVersion, parseFault, async ({ request, env, params, body }) => {
		const user = await sharer(request, env)
		const item = await readable(env, params.id, user.id)
		const version = await memberVersion(env, item, user.id, params.version)
		await reportVersion(env, item, version, user.id, await body())
	}),
	route(
		sharing.SetIcon,
		rawBody,
		async ({ request, env, params, body }) => {
			const user = await sharer(request, env)
			const item = await readable(env, params.id, user.id)
			owner(item, user.id)
			const image = await body()
			const upload = crypto.randomUUID()
			await env.FILES.put(iconKey(item.id, upload), await image.arrayBuffer(), {
				httpMetadata: { contentType: image.type || 'image/png' },
			})
			await setIcon(env, item, `${new URL(request.url).origin}/v1/icons/${item.id}/${upload}`, null)
		},
		ICON_BYTES,
	),
	route(sharing.SetGeneratedIcon, validIconConfig, async ({ request, env, params, body }) => {
		const user = await sharer(request, env)
		const item = await readable(env, params.id, user.id)
		owner(item, user.id)
		await setIcon(env, item, null, JSON.stringify(await body()))
	}),
	route(sharing.RemoveIcon, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		const item = await readable(env, params.id, user.id)
		owner(item, user.id)
		await setIcon(env, item, null, null)
	}),
	listHostedServers,
	// A ticket for one Core to read this share for the user, so a server they manage can follow
	// it. The Core sends it with its reads, and the backend checks the user can still read then.
	route(
		sharing.IssueReadTicket,
		(raw) => ({ core_id: string(object(raw).core_id, 'core_id', 64) }),
		async ({ request, env, params, body }) => {
			const user = await sharer(request, env)
			const item = await readable(env, params.id, user.id)
			if (item.quarantine) throw quarantined()
			return readTicket(env, user.id, item.id, (await body()).core_id)
		},
	),
	route(sharing.GetShareStorage, noBody, async ({ request, env }) => {
		const shares = await ownerShares(env, (await requireUser(request, env)).id)
		return {
			used: shares.reduce((sum, share) => sum + share.used, 0),
			allowance: ALLOWANCE_BYTES,
			shares,
		}
	}),
	route(sharing.GetBlacklistStatus, noBody, async ({ request, env, params }) => {
		await requireUser(request, env)
		return {
			blacklisted: !!(await env.DB.prepare('SELECT 1 FROM sharing_blacklist WHERE user_id = ?')
				.bind(params.user_id)
				.first()),
		}
	}),
	route(sharing.BlacklistUsers, parseUserIds, async ({ request, env, body }) => {
		await requireModerator(request, env)
		const ids = (await body()).user_ids
		if (ids.length)
			await env.DB.batch(
				ids.map((id) =>
					env.DB.prepare('INSERT OR IGNORE INTO sharing_blacklist (user_id) VALUES (?)').bind(id),
				),
			)
	}),
	route(sharing.UnblacklistUsers, parseUserIds, async ({ request, env, body }) => {
		await requireModerator(request, env)
		const ids = (await body()).user_ids
		if (ids.length)
			await env.DB.batch(
				ids.map((id) => env.DB.prepare('DELETE FROM sharing_blacklist WHERE user_id = ?').bind(id)),
			)
	}),
	route(sharing.DeleteModeratedFile, noBody, async ({ request, env, params }) => {
		await requireModerator(request, env)
		// Files are opaque to the backend, so quarantining the instance is what it can do.
		await env.DB.prepare('UPDATE shared_instances SET quarantine = 1 WHERE id = ?')
			.bind(params.id)
			.run()
	}),
	// The people who manage servers on the Core see who plays there, as its owner does.
	route(sharing.ListPlayers, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		const { core_id, server_id } = params
		const roles = await serverRoles(env, user.id, core_id, server_id)
		const manager = await env.DB.prepare(
			'SELECT 1 FROM core_access WHERE core_id = ? AND user_id = ?',
		)
			.bind(core_id, user.id)
			.first()
		if (!roles.coreOwner && !roles.shareOwner && !manager) throw notServerOwner()
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
			const failed: string[] = []
			for (const id of user_ids) {
				if (id === user.id) continue
				if (!(await canInviteToSharedInstance(env, user.id, id))) {
					failed.push(id)
					continue
				}
				await env.DB.prepare(
					"INSERT OR IGNORE INTO hosted_server_players (core_id,server_id,user_id,join_type) VALUES (?,?,?,'invite')",
				)
					.bind(core_id, server_id, id)
					.run()
			}
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
			await env.DB.batch(
				ids.map((id) =>
					env.DB.prepare(
						'DELETE FROM hosted_server_players WHERE core_id = ? AND server_id = ? AND user_id = ?',
					).bind(core_id, server_id, id),
				),
			)
		if (roles.server)
			await notifyMembers(env, roles.server.instance_id, 'shared_instance_users', user.id)
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

async function requireModerator(request: Request, env: Env) {
	const user = await requireUser(request, env)
	if (user.role !== 'admin' && user.role !== 'moderator')
		throw new ApiError(403, 'not_moderator', 'Moderator access is required')
}

/**
 * The shared instance routes. A Core key reaches only its own (`coreKeyRoutes`), dispatched before
 * any account check.
 */
export async function handleSharing(request: Request, env: Env): Promise<Response | null> {
	return (
		(await serve(publicRoutes, request, env)) ??
		(await serve(
			tokenFrom(request).startsWith('amc_') ? coreKeyRoutes : accountRoutes,
			request,
			env,
		))
	)
}

/** Where an uploaded icon is kept in R2. Under the share, so `sweepFiles` clears them with it. */
const iconKey = (share: string, upload: string) => `icons/${share}/${upload}`

/** Replaces the share's icon with an uploaded image's `url` or icon editor `config`, and drops
 * the image it had. */
async function setIcon(env: Env, item: InstanceRow, url: string | null, config: string | null) {
	await env.DB.prepare('UPDATE shared_instances SET icon = ?,icon_config = ? WHERE id = ?')
		.bind(url, config, item.id)
		.run()
	const upload = item.icon?.split('/').pop()
	if (upload) await env.FILES.delete(iconKey(item.id, upload))
}

const ICON_COLOR = /^#[0-9a-fA-F]{6}$/
const ICON_SYMBOL = /^[a-z0-9_]{1,64}$/

/** The icon editor settings, checked by the rules of `modrinth-content-management`'s
 * `IconConfig::validate`. */
function validIconConfig(value: unknown): SharedIconConfig {
	const invalid = () => new ApiError(400, 'invalid_input', 'Invalid icon config')
	if (typeof value !== 'object' || value === null) throw invalid()
	const { background, symbol } = value as { background?: unknown; symbol?: unknown }
	if (typeof symbol !== 'string' || !ICON_SYMBOL.test(symbol)) throw invalid()
	if (typeof background !== 'object' || background === null) throw invalid()
	const color = (field: unknown): field is string =>
		typeof field === 'string' && ICON_COLOR.test(field)
	const bg = background as Record<string, unknown>
	if (bg.type === 'color' && color(bg.value))
		return { background: { type: 'color', value: bg.value }, symbol }
	if (bg.type === 'linear-top-down-gradient' && color(bg.top_color) && color(bg.bottom_color))
		return {
			background: { type: bg.type, top_color: bg.top_color, bottom_color: bg.bottom_color },
			symbol,
		}
	throw invalid()
}

function iconConfig(item: InstanceRow): SharedIconConfig | null {
	return item.icon_config ? JSON.parse(item.icon_config) : null
}
