import { requireUnlocked, requireUser, tokenFrom } from '../common/auth'
import { randomId } from '../common/crypto'
import { ApiError, json, readBytes, readJson } from '../common/http'
import { notificationResponse, notifyMembers } from '../common/notifications'
import type { Env } from '../common/types'
import { requireListedCore } from '../cores'
import { canInviteToSharedInstance } from '../social/preferences'
import { notifyUser } from '../social/socket'
import {
	ALLOWANCE_BYTES,
	EMPTY_ROOT,
	MAX_MANIFEST_BODY,
	MAX_SHARE_BYTES,
	MAX_VERSION_FILES,
	type ManifestFile,
	UNCONFIRMED_TTL_MS,
	addedBytes,
	confirmedHolders,
	keptVersions,
	manifestFile,
	manifestKey,
	ownerShares,
	shareVersions,
	place,
	poolAccess,
	root,
	shareUsage,
	storeGrant,
	versionBytes,
	versionObjects,
} from './pool'

// Sharing metadata, as Modrinth's App expects it. The files themselves live on Cores; see pool.ts.
// Every version lists all of the share's files, so dropping older ones loses nothing.

type Instance = {
	id: string
	owner_id: string
	name: string
	icon: string | null
	quarantine: number
	installed_once: number
	home: string | null
	replica: string | null
	/** The Core whose key writes this share, for a server-first share; see `coreSharing`. */
	core_id: string | null
	server_id: string | null
}
type Version = {
	instance_id: string
	version: number
	root: string
	bytes: number
	added: number | null
	ready: number
	created: string
}
type Link = {
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
type SharedServer = {
	core_id: string
	server_id: string
	instance_id: string
	name: string
	/** Its game port; players join it at the Core's address (`joinAddress`). */
	port: number
}
/** A modpack file the owner turned off, which members remove after installing the modpack. */
type RemovedFile =
	| { type: 'version'; version_id: string }
	| { type: 'path'; parent: string; filename: string }
/** The manifest as R2 holds it: the upstream fields, the pool's file entries, and R. */
type Manifest = {
	modrinth_ids: string[]
	modpack_id: string | null
	removed_files: RemovedFile[]
	game_version: string
	loader: string
	loader_version: string
	root: string
	external_files: ManifestFile[]
}
const empty = () => new Response(null, { status: 204 })
const now = () => new Date().toISOString()
const quarantined = () =>
	new ApiError(403, 'forbidden', 'Shared instance is quarantined', 'quarantined')

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

async function instance(env: Env, id: string): Promise<Instance> {
	const result = await env.DB.prepare('SELECT * FROM shared_instances WHERE id = ?')
		.bind(id)
		.first<Instance>()
	if (!result)
		throw new ApiError(404, 'not_found', 'Shared instance was not found', 'instance_not_found')
	return result
}

/**
 * SQL true while the server `s` may pass its share on: its Core's owner still owns the share it
 * follows or is a joined member of it. A member who leaves or is kicked takes their servers'
 * players with them, even before `dropServers` deletes the rows.
 */
const LIVE_SERVER = `EXISTS (SELECT 1 FROM cores lc JOIN shared_instances li ON li.id = s.instance_id WHERE lc.id = s.core_id
	AND (li.owner_id = lc.owner_id OR EXISTS (SELECT 1 FROM shared_members lm WHERE lm.instance_id = li.id AND lm.user_id = lc.owner_id AND lm.joined_at IS NOT NULL)))`

/**
 * SQL joining a server's players (`p`) to the server (`s`), which names the share it follows.
 * Only servers that may pass it on (`LIVE_SERVER`), so everything players read goes through it.
 */
const PLAYERS = `server_players p JOIN shared_servers s ON s.core_id = p.core_id AND s.server_id = p.server_id AND ${LIVE_SERVER}`

/** Whether `userId` owns the share or has joined it as a member. Players are not members. */
async function joinedMember(env: Env, item: Instance, userId: string): Promise<boolean> {
	if (item.owner_id === userId) return true
	return !!(await env.DB.prepare(
		'SELECT 1 FROM shared_members WHERE instance_id = ? AND user_id = ? AND joined_at IS NOT NULL',
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
			'DELETE FROM shared_links WHERE instance_id = ? AND server_id IS NOT NULL AND core_id IN (SELECT id FROM cores WHERE owner_id = ?)',
		).bind(instanceId, ownerId),
		env.DB.prepare(
			'DELETE FROM shared_servers WHERE instance_id = ? AND core_id IN (SELECT id FROM cores WHERE owner_id = ?)',
		).bind(instanceId, ownerId),
	])
}

/**
 * A Core's server was deleted or stopped following an instance (`DELETE /core/players`): its
 * players and links go, and the share's members are told.
 */
export async function forgetServer(env: Env, coreId: string, serverId: string): Promise<void> {
	const server = await env.DB.prepare(
		'SELECT instance_id FROM shared_servers WHERE core_id = ? AND server_id = ?',
	)
		.bind(coreId, serverId)
		.first<{ instance_id: string }>()
	await env.DB.batch([
		env.DB.prepare('DELETE FROM shared_links WHERE core_id = ? AND server_id = ?').bind(
			coreId,
			serverId,
		),
		env.DB.prepare('DELETE FROM shared_servers WHERE core_id = ? AND server_id = ?').bind(
			coreId,
			serverId,
		),
	])
	if (server) await notifyMembers(env, server.instance_id, 'shared_instance_users')
}

/**
 * Whoever reads the share: its owner, its members, and the players of every server that follows
 * it. Pending ones too with `allowPending`, so an invite can be previewed.
 */
async function member(
	env: Env,
	item: Instance,
	userId: string,
	allowPending = false,
): Promise<void> {
	if (item.owner_id === userId) return
	const row = await env.DB.prepare(
		`SELECT EXISTS (SELECT 1 FROM shared_members WHERE instance_id = ?1 AND user_id = ?2 AND (?3 OR joined_at IS NOT NULL))
			OR EXISTS (SELECT 1 FROM ${PLAYERS} WHERE s.instance_id = ?1 AND p.user_id = ?2 AND (?3 OR p.joined_at IS NOT NULL)) AS reads`,
	)
		.bind(item.id, userId, allowPending ? 1 : 0)
		.first<{ reads: number }>()
	if (!row?.reads)
		throw new ApiError(401, 'unauthorized', 'Shared instance access was revoked', 'not_member')
}

function owner(item: Instance, userId: string) {
	if (item.owner_id !== userId)
		throw new ApiError(
			403,
			'forbidden',
			'Only the owner can change this shared instance',
			'not_owner',
		)
}

async function requireSharing(env: Env, userId: string): Promise<void> {
	if (
		await env.DB.prepare('SELECT 1 FROM sharing_blacklist WHERE user_id = ?').bind(userId).first()
	)
		throw new ApiError(403, 'forbidden', 'This account cannot use sharing', 'blacklisted')
}

/** The manifest of a version, read from R2 as text so the Worker never parses it. */
async function manifestText(env: Env, item: Pick<Version, 'instance_id' | 'version'>) {
	const object = await env.FILES.get(manifestKey(item.instance_id, item.version))
	if (!object)
		throw new ApiError(404, 'not_found', 'Shared version was not found', 'version_not_found')
	return object.text()
}

/**
 * A version as the App reads it: the stored manifest, spliced as text, plus what the pool adds.
 * A member of a ready version gets the confirmed holders and a fetch ticket; the owner of an
 * unconfirmed one gets the home and a store ticket, so a crashed push resumes; the owner also
 * sees how many members reported broken files.
 */
async function versionResponse(env: Env, item: Instance, version: Version, userId: string) {
	const manifest = await manifestText(env, version)
	const extra: Record<string, unknown> = { version: version.version, ready: version.ready === 1 }
	if (version.root !== EMPTY_ROOT) {
		if (version.ready) {
			const holders = await confirmedHolders(env, [item.home, item.replica], version.root)
			extra.pool = await poolAccess(env, holders, version.root)
		} else if (item.owner_id === userId && item.home) {
			extra.store = await storeGrant(env, item.home, version.root, version.bytes)
		}
	}
	if (item.owner_id === userId) {
		const reports = await env.DB.prepare(
			"SELECT COUNT(*) AS content FROM shared_reports WHERE instance_id = ? AND version = ? AND kind = 'content'",
		)
			.bind(item.id, version.version)
			.first<{ content: number }>()
		extra.reports = { content: reports?.content ?? 0 }
	}
	// The manifest text is an object; the dynamic fields are spliced in after its last field.
	return new Response(`${manifest.trimEnd().slice(0, -1)},${JSON.stringify(extra).slice(1)}`, {
		headers: { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' },
	})
}

/** The newest complete version, or the one being uploaded when none is complete yet. */
async function latestVersion(env: Env, id: string): Promise<Version> {
	const result = await env.DB.prepare(
		'SELECT * FROM shared_versions WHERE instance_id = ? ORDER BY ready DESC, version DESC LIMIT 1',
	)
		.bind(id)
		.first<Version>()
	if (!result)
		throw new ApiError(404, 'not_found', 'Shared instance has no version', 'version_not_found')
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
			await env.DB.prepare('SELECT version FROM shared_versions WHERE instance_id = ?')
				.bind(id)
				.all<{ version: number }>()
		).results.map((row) => manifestKey(id, row.version)),
	)
	const unused = stored.filter((key) => !used.has(key))
	if (unused.length) await env.FILES.delete(unused)
}

/**
 * Keeps the `kept` latest complete versions (all when null) and drops uploads abandoned before
 * the newest one, with their manifests. Deletes by name, so a push does not pay for listing the
 * instance's manifests. The Cores holding the files drop them at their next storage report.
 */
async function pruneVersions(env: Env, id: string, kept: number | null): Promise<void> {
	const { results } = await env.DB.prepare(
		`DELETE FROM shared_versions WHERE instance_id = ?1 AND (
			version < (SELECT MIN(version) FROM (SELECT version FROM shared_versions WHERE instance_id = ?1 AND ready = 1 ORDER BY version DESC LIMIT ?2))
			OR (ready = 0 AND version < (SELECT MAX(version) FROM shared_versions WHERE instance_id = ?1 AND ready = 1)))
		RETURNING version`,
	)
		.bind(id, kept ?? -1)
		.all<{ version: number }>()
	if (results.length) await env.FILES.delete(results.map((row) => manifestKey(id, row.version)))
}

async function notifyReadyInvites(env: Env, id: string): Promise<void> {
	const ready = await env.DB.prepare(
		'SELECT 1 FROM shared_versions WHERE instance_id = ? AND ready = 1 LIMIT 1',
	)
		.bind(id)
		.first()
	if (!ready) return
	const item = await instance(env, id)
	const members = (
		await env.DB.prepare(
			'SELECT user_id FROM shared_members WHERE instance_id = ? AND joined_at IS NULL AND notified = 0',
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
			table: 'shared_members',
			rows: 'instance_id = ?1 AND user_id = ?2 AND joined_at IS NULL',
			keys: [id, row.user_id],
		})),
		...players.map((row) => ({
			user: row.user_id,
			name: row.name,
			by: row.owner_id,
			table: 'server_players',
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
		'UPDATE shared_versions SET ready = 1 WHERE instance_id = ? AND version = ? AND ready = 0',
	)
		.bind(id, version)
		.run()
	if (!result.meta.changes) return
	await notifyReadyInvites(env, id)
	const item = await instance(env, id)
	await pruneVersions(env, id, await keptVersions(env, item.owner_id))
}

/**
 * The owner's App pushed: it sends the manifest with every file already encrypted and named, and
 * gets back the home Core and a store ticket to upload to. The version is ready once that Core
 * confirms. A push with the same files as an unconfirmed version resumes it; one with other files
 * waits until that version is confirmed or an hour old. `server_core` names the owner's Core
 * running a server of this instance, where the share is then homed (see `place`). A Core pushing
 * its own server's share passes itself as `core`, which is that server's Core.
 */
async function createVersion(
	request: Request,
	env: Env,
	item: Instance,
	userId: string,
	core: string | null = null,
): Promise<Response> {
	owner(item, userId)
	const text = new TextDecoder().decode(await readBytes(request, MAX_MANIFEST_BODY))
	let body: Record<string, unknown>
	try {
		body = object(JSON.parse(text))
	} catch {
		throw new ApiError(400, 'invalid_input', 'Invalid JSON')
	}
	const loader = string(body.loader, 'loader')
	if (!['vanilla', 'fabric', 'forge', 'quilt', 'neoforge'].includes(loader))
		throw new ApiError(400, 'invalid_input', 'Invalid loader')
	if (!Array.isArray(body.external_files))
		throw new ApiError(400, 'invalid_input', 'Invalid external_files')
	if (body.external_files.length > MAX_VERSION_FILES)
		throw new ApiError(
			413,
			'invalid_input',
			`A shared instance can hold at most ${MAX_VERSION_FILES} files`,
			'too_many_files',
		)
	const seen = new Set<string>()
	const external = body.external_files.map((value) => {
		const entry = object(value)
		const file_name = fileName(entry.file_name)
		const file_type = fileType(entry.file_type)
		const name = `${file_type}:${file_name}`
		if (seen.has(name)) throw new ApiError(400, 'invalid_input', 'Duplicate external file')
		seen.add(name)
		return manifestFile(entry, file_name, file_type)
	})
	const modpackId =
		body.modpack_id === undefined || body.modpack_id === null
			? null
			: string(body.modpack_id, 'modpack_id')
	const manifest: Manifest = {
		modrinth_ids: strings(body.modrinth_ids ?? [], 'modrinth_ids', 1000),
		modpack_id: modpackId,
		removed_files: removedFiles(body.removed_files, modpackId),
		game_version: string(body.game_version, 'game_version'),
		loader,
		loader_version: typeof body.loader_version === 'string' ? body.loader_version : '',
		root: await root(external.map((file) => file.object)),
		external_files: external,
	}
	if (external.length && body.root !== manifest.root)
		throw new ApiError(400, 'invalid_input', 'root does not match the files')
	const bytes = versionBytes(external)
	if (bytes > MAX_SHARE_BYTES)
		throw new ApiError(
			413,
			'invalid_input',
			'A shared instance can hold at most 1 GB of files',
			'share_too_large',
		)
	const serverCore =
		core ??
		(body.server_core === undefined || body.server_core === null
			? null
			: string(body.server_core, 'server_core', 64))

	// One push at a time: an unconfirmed version younger than its store ticket is still being
	// uploaded, unless this push is the same version again, which resumes it.
	const pending = await env.DB.prepare(
		'SELECT * FROM shared_versions WHERE instance_id = ? AND ready = 0 ORDER BY version DESC LIMIT 1',
	)
		.bind(item.id)
		.first<Version>()
	if (pending) {
		const fresh = Date.parse(pending.created) > Date.now() - UNCONFIRMED_TTL_MS
		if (fresh && pending.root === manifest.root) {
			await env.FILES.put(manifestKey(item.id, pending.version), JSON.stringify(manifest))
			return versionResponse(env, item, pending, userId)
		}
		if (fresh)
			throw new ApiError(
				409,
				'conflict',
				'Another push of this instance is still uploading. Try again when it finishes.',
				'push_in_flight',
			)
		await env.DB.prepare('DELETE FROM shared_versions WHERE instance_id = ? AND version = ?')
			.bind(item.id, pending.version)
			.run()
	}

	// Where the files go, and what they add to the owner's usage: the share's kept versions once
	// this one is ready and those beyond the owner's setting are dropped.
	let added = bytes
	if (bytes) {
		const members = await env.DB.prepare(
			`SELECT 1 FROM shared_members WHERE instance_id = ?1 AND join_type != 'owner' UNION ALL SELECT 1 FROM ${PLAYERS} WHERE s.instance_id = ?1 LIMIT 1`,
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
				'no_storage',
			)
		const ready = (
			await env.DB.prepare(
				'SELECT version, bytes, added FROM shared_versions WHERE instance_id = ? AND ready = 1 ORDER BY version',
			)
				.bind(item.id)
				.all<Pick<Version, 'version' | 'bytes' | 'added'>>()
		).results
		const previous = ready.at(-1)
		if (previous)
			added = addedBytes(
				new Map(external.map((file) => [file.object, file.object_size])),
				await versionObjects(env, item.id, previous.version),
			)
		const [shares, kept, own] = await Promise.all([
			ownerShares(env, userId),
			keptVersions(env, userId),
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
			if (others + shareUsage(kept === null ? after : after.slice(-kept)) > ALLOWANCE_BYTES)
				throw new ApiError(
					413,
					'invalid_input',
					`This push needs more than your ${ALLOWANCE_BYTES / 1024 ** 3} GB of shared storage. Delete old versions to free space.`,
					'storage_quota',
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
		'INSERT INTO shared_versions (instance_id,version,root,bytes,added,ready,created) SELECT ?1,COALESCE(m,0)+1,?2,?3,?6,0,?4 FROM (SELECT MAX(version) AS m FROM shared_versions WHERE instance_id = ?1) WHERE NOT EXISTS (SELECT 1 FROM shared_versions WHERE instance_id = ?1 AND ready = 0 AND created > ?5) RETURNING *',
	)
		.bind(
			item.id,
			manifest.root,
			bytes,
			now(),
			new Date(Date.now() - UNCONFIRMED_TTL_MS).toISOString(),
			added,
		)
		.first<Version>()
	if (!version)
		throw new ApiError(
			409,
			'conflict',
			'Another push of this instance is still uploading. Try again when it finishes.',
			'push_in_flight',
		)
	try {
		await env.FILES.put(manifestKey(item.id, version.version), JSON.stringify(manifest))
	} catch (error) {
		await env.DB.prepare('DELETE FROM shared_versions WHERE instance_id = ? AND version = ?')
			.bind(item.id, version.version)
			.run()
		throw error
	}
	// Ready at once with no files, or when the home already holds these exact objects.
	const held =
		bytes && home
			? await env.DB.prepare('SELECT 1 FROM core_holdings WHERE core_id = ? AND root = ?')
					.bind(home, manifest.root)
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
	request: Request,
	env: Env,
	item: Instance,
	version: Version,
	userId: string,
): Promise<Response> {
	const body = await readJson(request)
	const coreId = string(body.core_id, 'core_id', 64)
	const kind = string(body.kind, 'kind', 16)
	if (kind !== 'object' && kind !== 'content')
		throw new ApiError(400, 'invalid_input', 'Invalid kind')
	await env.DB.prepare(
		'INSERT OR REPLACE INTO shared_reports (instance_id,version,user_id,core_id,kind,created) VALUES (?,?,?,?,?,?)',
	)
		.bind(item.id, version.version, userId, coreId, kind, now())
		.run()
	return empty()
}

/**
 * The owner deletes an older version to free space. The current version, which members install,
 * stays, and so does one still uploading. The next version's added bytes are counted again
 * against the version before the deleted one, so usage stays right.
 */
async function deleteVersion(
	env: Env,
	item: Instance,
	version: Version,
	userId: string,
): Promise<Response> {
	owner(item, userId)
	const current = await latestVersion(env, item.id)
	if (!version.ready || current.version === version.version)
		throw new ApiError(
			409,
			'conflict',
			version.ready ? 'The current version cannot be deleted.' : 'This version is still uploading.',
			'current_version',
		)
	const neighbour = (direction: '<' | '>') =>
		env.DB.prepare(
			`SELECT version FROM shared_versions WHERE instance_id = ? AND ready = 1 AND version ${direction} ? ORDER BY version ${direction === '<' ? 'DESC' : 'ASC'} LIMIT 1`,
		)
			.bind(item.id, version.version)
			.first<{ version: number }>()
	const [previous, next] = await Promise.all([neighbour('<'), neighbour('>')])
	const remove = env.DB.prepare(
		'DELETE FROM shared_versions WHERE instance_id = ? AND version = ?',
	).bind(item.id, version.version)
	if (previous && next) {
		const [after, before] = await Promise.all([
			versionObjects(env, item.id, next.version),
			versionObjects(env, item.id, previous.version),
		])
		await env.DB.batch([
			remove,
			env.DB.prepare(
				'UPDATE shared_versions SET added = ? WHERE instance_id = ? AND version = ?',
			).bind(addedBytes(after, before), item.id, next.version),
		])
	} else await remove.run()
	await env.FILES.delete(manifestKey(item.id, version.version))
	return empty()
}

/**
 * `GET /v1/storage`: the account's usage of its allowance, its kept-versions setting, and its
 * shares with their kept versions. `PATCH` changes the setting; lowering it drops older versions
 * at once.
 */
async function storage(request: Request, env: Env, userId: string): Promise<Response> {
	if (request.method === 'PATCH') {
		const kept = (await readJson(request)).kept_versions
		if (
			kept !== null &&
			(typeof kept !== 'number' || !Number.isSafeInteger(kept) || kept < 1 || kept > 1000)
		)
			throw new ApiError(400, 'invalid_input', 'Invalid kept_versions')
		await env.DB.prepare(
			'INSERT INTO sharing_settings (user_id,kept_versions) VALUES (?,?) ON CONFLICT(user_id) DO UPDATE SET kept_versions = excluded.kept_versions',
		)
			.bind(userId, kept)
			.run()
		const owned = await env.DB.prepare('SELECT id FROM shared_instances WHERE owner_id = ?')
			.bind(userId)
			.all<{ id: string }>()
		for (const share of owned.results) await pruneVersions(env, share.id, kept)
	} else if (request.method !== 'GET')
		throw new ApiError(405, 'method_not_allowed', 'Unsupported storage operation')
	const [shares, kept] = await Promise.all([ownerShares(env, userId), keptVersions(env, userId)])
	return json({
		used: shares.reduce((sum, share) => sum + share.used, 0),
		allowance: ALLOWANCE_BYTES,
		kept_versions: kept,
		shares,
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

/** Marks a share installed once someone other than the owner joins it. Leaving never clears it. */
async function installedOnce(env: Env, item: Instance, userId: string): Promise<void> {
	if (item.installed_once || userId === item.owner_id) return
	await env.DB.prepare('UPDATE shared_instances SET installed_once = 1 WHERE id = ?')
		.bind(item.id)
		.run()
}

async function usableLink(env: Env, id: string): Promise<Link> {
	const link = await env.DB.prepare(
		'SELECT * FROM shared_links WHERE id = ? AND expiration > ? AND (max_uses = 0 OR uses < max_uses)',
	)
		.bind(id, now())
		.first<Link>()
	if (!link) throw new ApiError(404, 'not_found', 'Invite is invalid or expired', 'invite_invalid')
	return link
}

/**
 * `GET /v1/invites/{id}`, unauthenticated: what the link shares, shown before accepting. With
 * `?version=true` it adds the latest ready version for the install preview, without file keys or
 * pool access, which only members get. It has no version while none is ready or the instance is
 * quarantined.
 */
async function previewInvite(env: Env, id: string, withVersion: boolean): Promise<Response> {
	const link = await usableLink(env, id)
	const item = await instance(env, link.instance_id)
	const ready = await env.DB.prepare(
		'SELECT * FROM shared_versions WHERE instance_id = ? AND ready = 1 ORDER BY version DESC LIMIT 1',
	)
		.bind(item.id)
		.first<Version>()
	const manifest = ready ? (JSON.parse(await manifestText(env, ready)) as Manifest) : undefined
	// A server's link invites to the server, by its Core's owner: it shows nothing of the
	// instance's members, whom players never see.
	const server = link.server_id
		? await env.DB.prepare(
				'SELECT s.name, c.owner_id FROM shared_servers s JOIN cores c ON c.id = s.core_id WHERE s.core_id = ? AND s.server_id = ?',
			)
				.bind(link.core_id, link.server_id)
				.first<{ name: string; owner_id: string }>()
		: null
	if (link.server_id && !server)
		throw new ApiError(404, 'not_found', 'Invite was not found', 'invite_invalid')
	const manager = await env.DB.prepare('SELECT id,username,avatar_url FROM users WHERE id = ?')
		.bind(server?.owner_id ?? item.owner_id)
		.first<{ id: string; username: string; avatar_url: string | null }>()
	const users = server
		? []
		: (
				await env.DB.prepare(
					'SELECT u.id,u.username AS name,u.avatar_url AS avatar,m.joined_at FROM shared_members m JOIN users u ON u.id = m.user_id WHERE m.instance_id = ?',
				)
					.bind(item.id)
					.all()
			).results
	return json({
		instance_id: item.id,
		instance_name: server?.name ?? item.name,
		instance_icon: item.icon,
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
	})
}

async function invites(
	request: Request,
	env: Env,
	item: Instance,
	userId: string,
	inviteId?: string,
): Promise<Response> {
	// The invite to the share may have come as a member or as a player of a server following it;
	// accepting or declining answers every pending one.
	if (inviteId === 'pending') {
		if (request.method === 'POST') {
			const [members, players] = await env.DB.batch([
				env.DB.prepare(
					'UPDATE shared_members SET joined_at = COALESCE(joined_at,?1) WHERE instance_id = ?2 AND user_id = ?3',
				).bind(now(), item.id, userId),
				env.DB.prepare(
					'UPDATE server_players SET joined_at = COALESCE(joined_at,?1) WHERE user_id = ?3 AND (core_id, server_id) IN (SELECT core_id, server_id FROM shared_servers WHERE instance_id = ?2)',
				).bind(now(), item.id, userId),
			])
			if (!members?.meta.changes && !players?.meta.changes)
				throw new ApiError(404, 'not_found', 'Pending invite was not found', 'invite_not_found')
			await installedOnce(env, item, userId)
			await notifyMembers(env, item.id, 'shared_instance_users', userId)
			return empty()
		}
		if (request.method === 'DELETE') {
			const [members, players] = await env.DB.batch([
				env.DB.prepare(
					'DELETE FROM shared_members WHERE instance_id = ? AND user_id = ? AND joined_at IS NULL',
				).bind(item.id, userId),
				env.DB.prepare(
					'DELETE FROM server_players WHERE user_id = ?2 AND joined_at IS NULL AND (core_id, server_id) IN (SELECT core_id, server_id FROM shared_servers WHERE instance_id = ?1)',
				).bind(item.id, userId),
			])
			if (members?.meta.changes || players?.meta.changes)
				await notifyMembers(env, item.id, 'shared_instance_users', userId)
			return empty()
		}
	}
	if (inviteId && request.method === 'POST') {
		const link = await env.DB.prepare('SELECT * FROM shared_links WHERE id = ?')
			.bind(inviteId)
			.first<Link>()
		// A server's link makes them a player of that server; any other makes them a member.
		const joined = link?.server_id
			? await env.DB.prepare(
					'SELECT joined_at FROM server_players WHERE core_id = ? AND server_id = ? AND user_id = ?',
				)
					.bind(link.core_id, link.server_id, userId)
					.first<{ joined_at: string | null }>()
			: await env.DB.prepare(
					'SELECT joined_at FROM shared_members WHERE instance_id = ? AND user_id = ?',
				)
					.bind(item.id, userId)
					.first<{ joined_at: string | null }>()
		if (joined?.joined_at) return empty()
		const usable = await usableLink(env, inviteId)
		if (usable.instance_id !== item.id)
			throw new ApiError(404, 'not_found', 'Invite was not found', 'invite_invalid')
		await env.DB.batch([
			usable.server_id
				? env.DB.prepare(
						`INSERT INTO server_players (core_id,server_id,user_id,joined_at,join_type) SELECT l.core_id,l.server_id,?1,?2,'link' FROM shared_links l JOIN shared_servers s ON s.core_id = l.core_id AND s.server_id = l.server_id AND s.instance_id = l.instance_id WHERE ${LIVE_SERVER} AND l.id = ?3 AND l.expiration > ?2 AND (l.max_uses = 0 OR l.uses < l.max_uses) ON CONFLICT(core_id,server_id,user_id) DO UPDATE SET joined_at = excluded.joined_at,join_type = excluded.join_type`,
					).bind(userId, now(), inviteId)
				: env.DB.prepare(
						"INSERT INTO shared_members (instance_id,user_id,joined_at,join_type) SELECT instance_id,?,?,'link' FROM shared_links WHERE id = ? AND expiration > ? AND (max_uses = 0 OR uses < max_uses) ON CONFLICT(instance_id,user_id) DO UPDATE SET joined_at = excluded.joined_at,join_type = excluded.join_type",
					).bind(userId, now(), inviteId, now()),
			env.DB.prepare('UPDATE shared_links SET uses = uses + 1 WHERE id = ? AND changes() > 0').bind(
				inviteId,
			),
		])
		await member(env, item, userId)
		await installedOnce(env, item, userId)
		await notifyMembers(env, item.id, 'shared_instance_users', userId)
		return empty()
	}
	owner(item, userId)
	// A server's links are listed with its players (`serverPlayers`).
	if (request.method === 'GET' && !inviteId)
		return json(
			(
				await env.DB.prepare(
					'SELECT id,expiration,max_uses,uses FROM shared_links WHERE instance_id = ? AND server_id IS NULL AND expiration > ? ORDER BY expiration',
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
	if (request.method === 'POST' && !inviteId)
		return createLink(env, await readJson(request), item.id)
	throw new ApiError(405, 'method_not_allowed', 'Unsupported invite operation')
}

/** A new invite link to the share `instanceId`, or with `server`, to be a player of that server. */
async function createLink(
	env: Env,
	body: Record<string, unknown>,
	instanceId: string,
	server?: SharedServer,
): Promise<Response> {
	const id = inviteCode()
	const maxAge = natural(body.max_age, 604800, 31536000)
	const maxUses = natural(body.max_uses, 0, 10000)
	const expiration = new Date(Date.now() + (maxAge === 0 ? 31536000 : maxAge) * 1000).toISOString()
	await env.DB.prepare(
		'INSERT INTO shared_links (id,instance_id,expiration,max_uses,core_id,server_id) VALUES (?,?,?,?,?,?)',
	)
		.bind(id, instanceId, expiration, maxUses, server?.core_id ?? null, server?.server_id ?? null)
		.run()
	return json({ id })
}

async function users(
	request: Request,
	env: Env,
	item: Instance,
	userId: string,
): Promise<Response> {
	await member(env, item, userId)
	if (request.method === 'GET') {
		// Players get the content, not the list of who the instance is shared with.
		if (!(await joinedMember(env, item, userId)))
			throw new ApiError(
				403,
				'forbidden',
				'Only members see who an instance is shared with',
				'members_only',
			)
		const rows = (
			await env.DB.prepare(
				'SELECT user_id AS id,joined_at,join_type,last_played FROM shared_members WHERE instance_id = ?',
			)
				.bind(item.id)
				.all()
		).results
		const tokens = await env.DB.prepare(
			'SELECT COUNT(*) AS count FROM shared_links WHERE instance_id = ? AND server_id IS NULL AND expiration > ? AND (max_uses = 0 OR uses < max_uses)',
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
		if (ids.length) {
			await env.DB.batch([
				...ids.map((id) =>
					env.DB.prepare('DELETE FROM shared_members WHERE instance_id = ? AND user_id = ?').bind(
						item.id,
						id,
					),
				),
				// Leaving a share a player reads also leaves the servers it came through.
				...(ids.includes(userId)
					? [
							env.DB.prepare(
								'DELETE FROM server_players WHERE user_id = ?2 AND (core_id, server_id) IN (SELECT core_id, server_id FROM shared_servers WHERE instance_id = ?1)',
							).bind(item.id, userId),
						]
					: []),
				// A member who leaves or is kicked can no longer pass the share on through their servers.
				...dropServers(env, item.id, ids),
			])
			await notifyMembers(env, item.id, 'shared_instance_users', userId)
		}
		return empty()
	}
	owner(item, userId)
	if (request.method !== 'POST')
		throw new ApiError(405, 'method_not_allowed', 'Unsupported member operation')
	// Everyone who accepts sharing invitations is invited; the rest come back in `failed`.
	const failed: string[] = []
	for (const id of ids) {
		if (id === userId) continue
		if (!(await canInviteToSharedInstance(env, userId, id))) {
			failed.push(id)
			continue
		}
		await env.DB.prepare(
			"INSERT OR IGNORE INTO shared_members (instance_id,user_id,join_type) VALUES (?,?,'invite')",
		)
			.bind(item.id, id)
			.run()
	}
	await notifyReadyInvites(env, item.id)
	await notifyMembers(env, item.id, 'shared_instance_users', userId)
	return json({ failed })
}

/**
 * Record which share a Core server follows and what it is called, from the body of a players or
 * invites write: `instance_id`, `name`, and `port` (its game port). The share is the server's
 * own server-first share, or one the Core's owner owns or has joined; a player cannot pass on what
 * they were given. Changing the share drops the server's players and links, as the Core does when
 * the server follows another instance: they were given the old one.
 */
async function followShare(
	env: Env,
	userId: string,
	coreId: string,
	serverId: string,
	body: Record<string, unknown>,
): Promise<SharedServer> {
	const item = await instance(env, string(body.instance_id, 'instance_id', 64))
	const own = item.core_id === coreId && item.server_id === serverId
	const joined =
		own ||
		item.owner_id === userId ||
		(await env.DB.prepare(
			'SELECT 1 FROM shared_members WHERE instance_id = ? AND user_id = ? AND joined_at IS NOT NULL',
		)
			.bind(item.id, userId)
			.first())
	if ((item.core_id !== null && !own) || !joined)
		throw new ApiError(
			403,
			'forbidden',
			'A server can only follow your own share or one you joined',
			'not_member',
		)
	if (item.quarantine) throw quarantined()
	const server: SharedServer = {
		core_id: coreId,
		server_id: serverId,
		instance_id: item.id,
		name: string(body.name, 'name'),
		port: gamePort(body.port),
	}
	const followed = await env.DB.prepare(
		'SELECT instance_id FROM shared_servers WHERE core_id = ? AND server_id = ?',
	)
		.bind(coreId, serverId)
		.first<{ instance_id: string }>()
	if (followed && followed.instance_id !== item.id) await forgetServer(env, coreId, serverId)
	await env.DB.prepare(
		'INSERT INTO shared_servers (core_id,server_id,instance_id,name,port) VALUES (?,?,?,?,?) ON CONFLICT(core_id,server_id) DO UPDATE SET name = excluded.name, port = excluded.port',
	)
		.bind(coreId, serverId, item.id, server.name, server.port)
		.run()
	return server
}

function gamePort(value: unknown): number {
	if (typeof value !== 'number' || !Number.isInteger(value) || value < 1 || value > 65535)
		throw new ApiError(400, 'invalid_input', 'Invalid port')
	return value
}

/**
 * Where players join a server: the host of its Core's last reported address, the one reached from
 * outside when it has one, with the server's port. Read each time, so it follows the Core's reports.
 */
function joinAddress(server: { port: number; address: string; public_address: string | null }) {
	return `${new URL(server.public_address ?? server.address).hostname}:${server.port}`
}

/** The server a player reads the share through, with its Core's addresses; null for its owner and members. */
async function cameThrough(env: Env, item: Instance, userId: string) {
	if (item.owner_id === userId) return null
	return env.DB.prepare(
		`SELECT s.*, c.address, c.public_address FROM ${PLAYERS} JOIN cores c ON c.id = s.core_id WHERE s.instance_id = ?1 AND p.user_id = ?2 AND NOT EXISTS (SELECT 1 FROM shared_members WHERE instance_id = ?1 AND user_id = ?2) ORDER BY p.joined_at IS NULL, p.joined_at LIMIT 1`,
	)
		.bind(item.id, userId)
		.first<SharedServer & { address: string; public_address: string | null }>()
}

/**
 * `/v1/servers/{core}/{server}/players` and `/invites[/{id}]`, from the App's Play tab: the
 * players of a Core server, the people it is shared with. They read the share the server follows
 * without being its members, and get its updates until they leave or are removed. The Core's owner
 * invites them, directly or by link, and removes them; so can the owner of the share the server
 * follows, from that instance's Share tab (`access`). Each write also says which share the server
 * follows (`followShare`). Answers in the shapes of the share's own `users` and `invites`.
 */
async function serverPlayers(
	request: Request,
	env: Env,
	userId: string,
	coreId: string,
	serverId: string,
	section: string,
	child: string | undefined,
): Promise<Response> {
	const core = await env.DB.prepare('SELECT owner_id FROM cores WHERE id = ?')
		.bind(coreId)
		.first<{ owner_id: string }>()
	if (!core) throw new ApiError(404, 'not_found', 'Core was not found', 'core_not_found')
	const server = await env.DB.prepare(
		'SELECT * FROM shared_servers WHERE core_id = ? AND server_id = ?',
	)
		.bind(coreId, serverId)
		.first<SharedServer>()
	const coreOwner = core.owner_id === userId
	const shareOwner = server ? (await instance(env, server.instance_id)).owner_id === userId : false
	const refused = () =>
		new ApiError(403, 'forbidden', "Only the server's owner can share it", 'not_owner')
	const links = 'FROM shared_links WHERE core_id = ? AND server_id = ? AND expiration > ?'

	if (section === 'players' && !child && request.method === 'GET') {
		// The people who manage servers on the Core see who plays there, as its owner does.
		const manager = await env.DB.prepare(
			'SELECT 1 FROM core_access WHERE core_id = ? AND user_id = ?',
		)
			.bind(coreId, userId)
			.first()
		if (!coreOwner && !shareOwner && !manager) throw refused()
		const users = (
			await env.DB.prepare(
				'SELECT user_id AS id, joined_at, join_type, NULL AS last_played FROM server_players WHERE core_id = ? AND server_id = ? ORDER BY joined_at IS NULL, joined_at',
			)
				.bind(coreId, serverId)
				.all()
		).results
		const tokens = await env.DB.prepare(
			`SELECT COUNT(*) AS count ${links} AND (max_uses = 0 OR uses < max_uses)`,
		)
			.bind(coreId, serverId, now())
			.first<{ count: number }>()
		return json({ users, tokens: tokens?.count ?? 0 })
	}
	if (section === 'players' && !child && request.method === 'DELETE') {
		const ids = strings((await readJson(request)).user_ids, 'user_ids', 100)
		// A player may leave; the Core's owner and the share's owner remove anyone.
		if (!coreOwner && !shareOwner && ids.some((id) => id !== userId)) throw refused()
		if (ids.length)
			await env.DB.batch(
				ids.map((id) =>
					env.DB.prepare(
						'DELETE FROM server_players WHERE core_id = ? AND server_id = ? AND user_id = ?',
					).bind(coreId, serverId, id),
				),
			)
		if (server) await notifyMembers(env, server.instance_id, 'shared_instance_users', userId)
		return empty()
	}
	if (!coreOwner) throw refused()
	if (section === 'invites' && !child && request.method === 'GET')
		return json(
			(
				await env.DB.prepare(`SELECT id,expiration,max_uses,uses ${links} ORDER BY expiration`)
					.bind(coreId, serverId, now())
					.all()
			).results,
		)
	if (section === 'invites' && child && request.method === 'DELETE') {
		await env.DB.prepare('DELETE FROM shared_links WHERE id = ? AND core_id = ? AND server_id = ?')
			.bind(child, coreId, serverId)
			.run()
		return empty()
	}
	if (child || request.method !== 'POST')
		throw new ApiError(405, 'method_not_allowed', 'Unsupported player operation')
	const body = await readJson(request)
	const followed = await followShare(env, userId, coreId, serverId, body)
	if (section === 'invites') return createLink(env, body, followed.instance_id, followed)
	// Everyone who accepts sharing invitations is invited; the rest come back in `failed`.
	const failed: string[] = []
	for (const id of strings(body.user_ids, 'user_ids', 100)) {
		if (id === userId) continue
		if (!(await canInviteToSharedInstance(env, userId, id))) {
			failed.push(id)
			continue
		}
		await env.DB.prepare(
			"INSERT OR IGNORE INTO server_players (core_id,server_id,user_id,join_type) VALUES (?,?,?,'invite')",
		)
			.bind(coreId, serverId, id)
			.run()
	}
	await notifyReadyInvites(env, followed.instance_id)
	await notifyMembers(env, followed.instance_id, 'shared_instance_users', userId)
	return json({ failed })
}

/**
 * `GET /v1/instances/{id}/access`, owner only, for the instance's Share tab: everyone who gets the
 * share's updates and how, and the live links of the servers that follow it. `users` lists
 * members, then the players of each server, named by that server; someone is listed once for
 * each way in. A member is kicked with `users`, a player with their server's `players`, and a
 * server's link revoked with the share's own `DELETE invites/{id}`.
 */
async function access(env: Env, item: Instance, userId: string): Promise<Response> {
	owner(item, userId)
	type Person = {
		id: string
		username: string
		avatar_url: string | null
		joined_at: string | null
		join_type: 'invite' | 'link'
	}
	type ServerRow = { core_id: string; server_id: string; name: string }
	const [members, players, links] = await Promise.all([
		env.DB.prepare(
			"SELECT u.id, u.username, u.avatar_url, m.joined_at, m.join_type FROM shared_members m JOIN users u ON u.id = m.user_id WHERE m.instance_id = ? AND m.join_type != 'owner' ORDER BY m.joined_at IS NULL, m.joined_at",
		)
			.bind(item.id)
			.all<Person>(),
		env.DB.prepare(
			`SELECT u.id, u.username, u.avatar_url, p.joined_at, p.join_type, s.core_id, s.server_id, s.name FROM ${PLAYERS} JOIN users u ON u.id = p.user_id WHERE s.instance_id = ? ORDER BY s.name, p.joined_at IS NULL, p.joined_at`,
		)
			.bind(item.id)
			.all<Person & ServerRow>(),
		env.DB.prepare(
			'SELECT l.id, l.expiration, l.max_uses, l.uses, s.core_id, s.server_id, s.name FROM shared_links l JOIN shared_servers s ON s.core_id = l.core_id AND s.server_id = l.server_id WHERE l.instance_id = ? AND l.expiration > ? ORDER BY s.name, l.expiration',
		)
			.bind(item.id, now())
			.all<{ id: string; expiration: string; max_uses: number; uses: number } & ServerRow>(),
	])
	const entry = ({ id, username, avatar_url, joined_at, join_type }: Person) => ({
		user: { id, username, avatar_url },
		joined_at,
		join_type,
	})
	const server = ({ core_id, server_id, name }: ServerRow) => ({ core_id, server_id, name })
	return json({
		users: [
			...members.results.map((row) => ({ ...entry(row), via: { type: 'member' } })),
			...players.results.map((row) => ({
				...entry(row),
				via: { type: 'player', ...server(row) },
			})),
		],
		links: links.results.map(({ id, expiration, max_uses, uses, ...row }) => ({
			id,
			expiration,
			max_uses,
			uses,
			server: server(row),
		})),
	})
}

/**
 * A Core, with its key, writing the share of one of its server-first servers (a server that
 * follows no instance). It acts as the share's owner, its own owner account, for: create
 * (`POST /v1/instances` with `server_id`, answering the server's existing share if it has one),
 * rename, delete, and its versions. It reaches no other share and no other route.
 */
async function coreSharing(
	request: Request,
	env: Env,
	[route, id, section, child, ...rest]: string[],
): Promise<Response> {
	const core = await requireListedCore(request, env)
	const refused = () =>
		new ApiError(403, 'forbidden', 'A Core reaches only the shares of its own servers', 'not_owner')
	if (route !== 'instances') throw refused()
	// An owner may always delete their share, locked or not.
	if (!(id && !section && request.method === 'DELETE')) {
		await requireUnlocked(env, core.owner_id)
		await requireSharing(env, core.owner_id)
	}
	if (!id) {
		if (request.method !== 'POST') throw refused()
		const body = await readJson(request)
		const name = string(body.name, 'name')
		const serverId = string(body.server_id, 'server_id', 64)
		await env.DB.prepare(
			'INSERT INTO shared_instances (id,owner_id,name,created,core_id,server_id) VALUES (?,?,?,?,?,?) ON CONFLICT DO NOTHING',
		)
			.bind(crypto.randomUUID(), core.owner_id, name, now(), core.id, serverId)
			.run()
		const row = await env.DB.prepare(
			'SELECT id FROM shared_instances WHERE core_id = ? AND server_id = ? AND owner_id = ?',
		)
			.bind(core.id, serverId, core.owner_id)
			.first<{ id: string }>()
		if (!row) throw refused()
		return json({ id: row.id })
	}
	const item = await instance(env, id)
	// The owner check matters because a Core picks its own id when it pairs: a share left from an
	// earlier pairing of that id is not this Core's.
	if (item.core_id !== core.id || item.owner_id !== core.owner_id) throw refused()
	if (!section && request.method === 'PATCH') {
		await env.DB.prepare('UPDATE shared_instances SET name = ? WHERE id = ?')
			.bind(string((await readJson(request)).name, 'name'), id)
			.run()
		return empty()
	}
	if (!section && request.method === 'DELETE') {
		await env.DB.prepare('DELETE FROM shared_instances WHERE id = ?').bind(id).run()
		await sweepFiles(env, id)
		return empty()
	}
	if (section !== 'versions' || rest.length) throw refused()
	if (item.quarantine) throw quarantined()
	if (request.method === 'POST' && !child)
		return createVersion(request, env, item, core.owner_id, core.id)
	if (request.method !== 'GET') throw refused()
	if (child && (!/^\d+$/.test(child) || !Number.isSafeInteger(Number(child))))
		throw new ApiError(400, 'invalid_input', 'Invalid shared version')
	const version = child
		? await env.DB.prepare('SELECT * FROM shared_versions WHERE instance_id = ? AND version = ?')
				.bind(id, Number(child))
				.first<Version>()
		: await latestVersion(env, id)
	if (!version)
		throw new ApiError(404, 'not_found', 'Shared version was not found', 'version_not_found')
	return versionResponse(env, item, version, core.owner_id)
}

export async function handleSharing(request: Request, env: Env): Promise<Response | null> {
	const url = new URL(request.url)
	const parts = url.pathname.split('/').filter(Boolean).map(decodeURIComponent)
	if (
		parts[0] !== 'v1' ||
		!['instances', 'invites', 'blacklist', 'moderation', 'icons', 'storage', 'servers'].includes(
			parts[1] ?? '',
		)
	)
		return null
	const [, route, id, section, child] = parts
	if (route === 'invites' && id && request.method === 'GET')
		return previewInvite(env, id, url.searchParams.get('version') === 'true')
	if (route === 'icons' && id && request.method === 'GET') {
		const row = await env.DB.prepare(
			'SELECT icon_data,icon_type FROM shared_instances WHERE icon = ?',
		)
			.bind(url.origin + url.pathname)
			.first<{ icon_data: string | null; icon_type: string | null }>()
		if (!row?.icon_data)
			throw new ApiError(404, 'not_found', 'Icon was not found', 'icon_not_found')
		return new Response(
			Uint8Array.from(atob(row.icon_data), (character) => character.charCodeAt(0)),
			{
				headers: {
					'Content-Type': row.icon_type ?? 'image/png',
					// Every upload gets a new URL, so a fetched icon never changes.
					'Cache-Control': 'private, max-age=31536000, immutable',
				},
			},
		)
	}
	if (tokenFrom(request).startsWith('amc_')) return coreSharing(request, env, parts.slice(1))
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
			throw new ApiError(403, 'forbidden', 'Moderator access is required', 'not_moderator')
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
			// Files are opaque to the backend, so quarantining the instance is what it can do.
			await env.DB.prepare('UPDATE shared_instances SET quarantine = 1 WHERE id = ?')
				.bind(section)
				.run()
			return empty()
		}
	}
	if (route === 'storage' && !id) return storage(request, env, user.id)
	if (route === 'servers' && id && section && (child === 'players' || child === 'invites')) {
		await requireSharing(env, user.id)
		return serverPlayers(request, env, user.id, id, section, child, parts[5])
	}
	if (route !== 'instances')
		throw new ApiError(404, 'not_found', 'Sharing endpoint was not found', 'endpoint_not_found')
	// An owner may always delete their share; `owner` below still checks it is theirs.
	if (!(id && !section && request.method === 'DELETE')) await requireSharing(env, user.id)
	if (!id) {
		if (request.method === 'GET') {
			if (url.searchParams.has('user') && url.searchParams.get('user') !== user.id)
				throw new ApiError(
					403,
					'forbidden',
					'Private instance lists belong to their account',
					'not_own_list',
				)
			return json(
				(
					await env.DB.prepare(
						`SELECT instance_id FROM shared_members WHERE user_id = ?1 AND joined_at IS NOT NULL UNION SELECT s.instance_id FROM ${PLAYERS} WHERE p.user_id = ?1 AND p.joined_at IS NOT NULL`,
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
	if (section === 'access' && !child && request.method === 'GET') return access(env, item, user.id)
	await member(env, item, user.id, !section)
	if (!section) {
		if (request.method === 'GET') {
			const server = await cameThrough(env, item, user.id)
			return json({
				name: item.name,
				icon: item.icon,
				quarantine: item.quarantine === 1,
				installed_once: item.installed_once === 1,
				// The App adds it to the instance's server list.
				linked_server: server
					? { domain: joinAddress(server), region: '', online_status: null }
					: null,
			})
		}
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
	// The backend keeps no server icons yet, so the server a player came through has none. The App
	// reads 404 as no icon.
	if (section === 'server-icon' && !child && request.method === 'GET')
		throw new ApiError(404, 'not_found', 'Shared instance has no server icon', 'icon_not_found')
	if (section === 'history' && !child && request.method === 'GET') {
		if (item.quarantine) throw quarantined()
		return json(await shareVersions(env, id))
	}
	if (section === 'versions') {
		if (item.quarantine) throw quarantined()
		if (request.method === 'POST' && !child) return createVersion(request, env, item, user.id)
		if (child && (!/^\d+$/.test(child) || !Number.isSafeInteger(Number(child))))
			throw new ApiError(400, 'invalid_input', 'Invalid shared version')
		const version = child
			? await env.DB.prepare('SELECT * FROM shared_versions WHERE instance_id = ? AND version = ?')
					.bind(id, Number(child))
					.first<Version>()
			: await latestVersion(env, id)
		// Members only see versions once they are ready; the App reads the 404 as "never ready".
		if (!version || (child && !version.ready && item.owner_id !== user.id))
			throw new ApiError(404, 'not_found', 'Shared version was not found', 'version_not_found')
		if (request.method === 'GET' && parts.length <= 5)
			return versionResponse(env, item, version, user.id)
		if (request.method === 'DELETE' && child && parts.length === 5)
			return deleteVersion(env, item, version, user.id)
		if (request.method === 'POST' && parts[5] === 'reports' && parts.length === 6)
			return reportVersion(request, env, item, version, user.id)
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
