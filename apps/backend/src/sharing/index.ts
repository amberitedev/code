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
type Link = { id: string; instance_id: string; expiration: string; max_uses: number; uses: number }
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

/** A file of the version's modpack that members do not get, as `SharedInstanceRemovedFile`. */
type RemovedFile =
	| { type: 'version'; version_id: string }
	| { type: 'path'; parent: string; filename: string }
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

function removedFiles(value: unknown, modpack: string | null): RemovedFile[] {
	if (value === undefined || value === null) return []
	if (!Array.isArray(value) || value.length > 1000 || (value.length && !modpack))
		throw new ApiError(400, 'invalid_input', 'Invalid removed_files')
	return value.map((value): RemovedFile => {
		const entry = object(value)
		if (entry.type === 'version')
			return { type: 'version', version_id: string(entry.version_id, 'version_id') }
		const filename = fileName(entry.filename)
		if (
			entry.type !== 'path' ||
			typeof entry.parent !== 'string' ||
			!REMOVED_PARENTS.includes(entry.parent) ||
			!safeFileName(filename) ||
			!REMOVED_SUFFIXES.some((suffix) => filename.endsWith(suffix))
		)
			throw new ApiError(400, 'invalid_input', 'Invalid removed_files')
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
	const modpack =
		body.modpack_id === undefined || body.modpack_id === null
			? null
			: string(body.modpack_id, 'modpack_id')
	const manifest: Manifest = {
		modrinth_ids: strings(body.modrinth_ids ?? [], 'modrinth_ids', 1000),
		modpack_id: modpack,
		removed_files: removedFiles(body.removed_files, modpack),
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
			"SELECT 1 FROM shared_members WHERE instance_id = ? AND join_type != 'owner' LIMIT 1",
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

async function usableLink(env: Env, id: string): Promise<Link> {
	const link = await env.DB.prepare(
		'SELECT * FROM shared_links WHERE id = ? AND expiration > ? AND (max_uses = 0 OR uses < max_uses)',
	)
		.bind(id, now())
		.first<Link>()
	if (!link) throw new ApiError(404, 'not_found', 'Invite is invalid or expired', 'invite_invalid')
	return link
}

async function previewInvite(env: Env, id: string): Promise<Response> {
	const link = await usableLink(env, id)
	const item = await instance(env, link.instance_id)
	const version = await latestVersion(env, item.id)
	const manifest = JSON.parse(await manifestText(env, version)) as Manifest
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
			if (!result.meta.changes)
				throw new ApiError(404, 'not_found', 'Pending invite was not found', 'invite_not_found')
			await notifyMembers(env, item.id, 'shared_instance_users', userId)
			return empty()
		}
		if (request.method === 'DELETE') {
			const declined = await env.DB.prepare(
				'DELETE FROM shared_members WHERE instance_id = ? AND user_id = ? AND joined_at IS NULL',
			)
				.bind(item.id, userId)
				.run()
			if (declined.meta.changes) await notifyMembers(env, item.id, 'shared_instance_users', userId)
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
		if (link.instance_id !== item.id)
			throw new ApiError(404, 'not_found', 'Invite was not found', 'invite_invalid')
		await env.DB.batch([
			env.DB.prepare(
				"INSERT INTO shared_members (instance_id,user_id,joined_at,join_type) SELECT instance_id,?,?,'link' FROM shared_links WHERE id = ? AND expiration > ? AND (max_uses = 0 OR uses < max_uses) ON CONFLICT(instance_id,user_id) DO UPDATE SET joined_at = excluded.joined_at,join_type = excluded.join_type",
			).bind(userId, now(), inviteId, now()),
			env.DB.prepare('UPDATE shared_links SET uses = uses + 1 WHERE id = ? AND changes() > 0').bind(
				inviteId,
			),
		])
		await member(env, item, userId)
		await notifyMembers(env, item.id, 'shared_instance_users', userId)
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
		if (ids.length) {
			await env.DB.batch(
				ids.map((id) =>
					env.DB.prepare('DELETE FROM shared_members WHERE instance_id = ? AND user_id = ?').bind(
						item.id,
						id,
					),
				),
			)
			await notifyMembers(env, item.id, 'shared_instance_users', userId)
		}
		return empty()
	}
	owner(item, userId)
	if (request.method !== 'POST')
		throw new ApiError(405, 'method_not_allowed', 'Unsupported member operation')
	for (const id of ids) {
		if (id === userId) continue
		if (!(await canInviteToSharedInstance(env, userId, id)))
			throw new ApiError(
				403,
				'forbidden',
				'This account does not accept this sharing invitation',
				'invite_not_accepted',
			)
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
	await notifyMembers(env, item.id, 'shared_instance_users', userId)
	return empty()
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
		!['instances', 'invites', 'blacklist', 'moderation', 'icons', 'storage'].includes(
			parts[1] ?? '',
		)
	)
		return null
	const [, route, id, section, child] = parts
	if (route === 'invites' && id && request.method === 'GET') return previewInvite(env, id)
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
		if (!version)
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
