import { isPrivateHost } from '../common/address'
import { ApiError } from '../common/http'
import type { Env } from '../common/types'
import { type Holder, fetchTicket, storeTicket } from '../cores/tickets'

// The pool side of sharing: which Core holds a share, the manifest an owner's App sends, and the
// tickets that let Apps talk to stores. See docs/internals/pool.md, "Protocol". The backend never
// carries a file byte: it keeps manifests in R2, one row per version in D1, and signs tickets.

export const MAX_VERSION_FILES = 4096
/** The manifest request body; 4,096 entries with keys are about 1 MB. */
export const MAX_MANIFEST_BODY = 4 * 1024 * 1024
/** A share's latest version, in distinct encrypted bytes. */
export const MAX_SHARE_BYTES = 1024 ** 3
/** Every account's allowance on other people's Cores: its usage may not pass this. */
export const ALLOWANCE_BYTES = 3 * 1024 ** 3
/** Ready versions a share keeps, files included, when its owner has not chosen. */
export const DEFAULT_KEPT_VERSIONS = 5
/** How long an unconfirmed version blocks the next push: the store ticket's life. */
export const UNCONFIRMED_TTL_MS = 60 * 60_000
/** A Core that has not reported its storage for this long takes no new shares. */
const REPORT_FRESH_MS = 2 * 86_400_000
/** R of a version with no files: SHA-256 of nothing. */
export const EMPTY_ROOT = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
/** Files encrypted with a key derived from their own content, so one copy serves every share. */
const CONTENT_TYPES = new Set(['mod', 'resourcepack', 'shader', 'datapack'])
const HASH = /^[0-9a-f]{64}$/
const SHA1 = /^[0-9a-f]{40}$/

export const manifestKey = (instanceId: string, version: number) =>
	`manifests/${instanceId}/${version}`

/**
 * One `external_files` entry. The first fields are Modrinth's: `file_size` is the plain size and
 * `sha1` is what a member's install compares to skip a file it already has. The rest are the
 * pool's: P, C, the encrypted size, and the key of a non-content file.
 */
export type ManifestFile = {
	file_name: string
	file_type: string
	/** `pool:<C>`, for upstream code that expects a URL; nothing fetches it. */
	url: string
	file_size: number
	sha1: string
	sha256: string
	object: string
	object_size: number
	key?: string
}

/** One `external_files` entry as the owner's App sends it. `name` is already validated. */
export function manifestFile(
	entry: Record<string, unknown>,
	file_name: string,
	file_type: string,
): ManifestFile {
	const count = (field: string) => {
		const value = entry[field]
		if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0)
			throw new ApiError(400, 'invalid_input', `Invalid ${field} for ${file_name}`)
		return value
	}
	const hash = (field: string, pattern = HASH) => {
		const value = entry[field]
		if (typeof value !== 'string' || !pattern.test(value))
			throw new ApiError(400, 'invalid_input', `Invalid ${field} for ${file_name}`)
		return value
	}
	const size = count('file_size')
	const object_size = count('object_size')
	// Every chunk of 1 MiB, and the last one even when empty, carries a 16-byte tag.
	if (object_size !== size + 16 * Math.max(1, Math.ceil(size / 1_048_576)))
		throw new ApiError(400, 'invalid_input', `Invalid object_size for ${file_name}`)
	const object = hash('object')
	const file: ManifestFile = {
		file_name,
		file_type,
		url: `pool:${object}`,
		file_size: size,
		sha1: hash('sha1', SHA1),
		sha256: hash('sha256'),
		object,
		object_size,
	}
	if (CONTENT_TYPES.has(file_type)) {
		if (entry.key !== undefined)
			throw new ApiError(400, 'invalid_input', `${file_name} must not carry a key`)
	} else file.key = hash('key')
	return file
}

const hex = (bytes: ArrayBuffer) =>
	Array.from(new Uint8Array(bytes), (byte) => byte.toString(16).padStart(2, '0')).join('')

/** R: the hash of the distinct object hashes, sorted, as raw bytes. */
export async function root(objects: Iterable<string>): Promise<string> {
	const sorted = [...new Set(objects)].sort()
	const bytes = new Uint8Array(sorted.length * 32)
	sorted.forEach((object, index) => {
		for (let byte = 0; byte < 32; byte++)
			bytes[index * 32 + byte] = Number.parseInt(object.slice(byte * 2, byte * 2 + 2), 16)
	})
	return hex(await crypto.subtle.digest('SHA-256', bytes))
}

/** The distinct object bytes of a version. */
export function versionBytes(files: ManifestFile[]): number {
	const seen = new Map<string, number>()
	for (const file of files) seen.set(file.object, file.object_size)
	let total = 0
	for (const size of seen.values()) total += size
	return total
}

const HOLDER_COLUMNS = 'id, address, public_address, cert_fingerprint'

type Candidate = Holder & { owner_id: string; room: number }

/**
 * Cores that may take a new share: storage on, running as a service with a public address that is
 * not on a private network (neither required in local development), room under their cap, and a
 * storage report within two days.
 */
async function eligibleCores(env: Env): Promise<Candidate[]> {
	const local = env.LOCAL_DEV === 'true'
	const rows = await env.DB.prepare(
		`SELECT c.${HOLDER_COLUMNS.replaceAll(', ', ', c.')}, c.owner_id, s.cap - s.used AS room FROM cores c JOIN core_storage s ON s.core_id = c.id WHERE s.enabled = 1 AND s.cap > s.used AND s.reported > ?1 AND (?2 OR (s.service = 1 AND c.public_address IS NOT NULL)) AND c.cert_fingerprint IS NOT NULL`,
	)
		.bind(new Date(Date.now() - REPORT_FRESH_MS).toISOString(), local ? 1 : 0)
		.all<Candidate>()
	if (local) return rows.results
	return rows.results.filter(
		(core) => core.public_address && !isPrivateHost(new URL(core.public_address).hostname),
	)
}

/** A share's two copies. Either is null while it has none. */
export type Placement = { home: string | null; replica: string | null }

/** One of `cores` drawn by free space, so a new large disk does not take every share at once. */
function draw(cores: Candidate[], exclude: string[]): string | null {
	const open = cores.filter((core) => !exclude.includes(core.id))
	const total = open.reduce((sum, core) => sum + core.room, 0)
	if (!total) return null
	let left = Math.random() * total
	for (const core of open) {
		left -= core.room
		if (left < 0) return core.id
	}
	return open[open.length - 1]?.id ?? null
}

/**
 * Where a share's two copies go (docs/internals/pool.md, "Where a share lives"). An owner with a
 * private Core, one with storage off, keeps every share there alone. Otherwise a share whose
 * instance has a server on `serverCore`, one of the owner's Cores, is homed there, and the rest
 * live on other people's Cores. A share with no members takes no replica: only its server reads
 * it. Copies on a Core that still qualifies stay put, so a push moves nothing it need not.
 * `home` is null when no Core can take the share.
 */
export async function place(
	env: Env,
	ownerId: string,
	current: Placement,
	{ serverCore, members }: { serverCore: string | null; members: boolean },
): Promise<Placement> {
	const own = (
		await env.DB.prepare(
			'SELECT c.id, s.enabled FROM cores c JOIN core_storage s ON s.core_id = c.id WHERE c.owner_id = ? AND s.reported > ? AND c.cert_fingerprint IS NOT NULL',
		)
			.bind(ownerId, new Date(Date.now() - REPORT_FRESH_MS).toISOString())
			.all<{ id: string; enabled: number }>()
	).results
	const privateCores = own.filter((core) => !core.enabled).map((core) => core.id)
	if (privateCores.length) {
		const home =
			[serverCore, current.home].find((id) => id && privateCores.includes(id)) ?? privateCores[0]
		return { home: home ?? null, replica: null }
	}
	const server = own.find((core) => core.id === serverCore)?.id ?? null
	const held = [current.home, current.replica].filter((id): id is string => id !== null)
	const offering = held.length
		? new Set(
				(
					await env.DB.prepare(
						'SELECT c.id FROM cores c JOIN core_storage s ON s.core_id = c.id WHERE s.enabled = 1 AND c.owner_id != ? AND c.id IN (SELECT value FROM json_each(?))',
					)
						.bind(ownerId, JSON.stringify(held))
						.all<{ id: string }>()
				).results.map((row) => row.id),
			)
		: new Set<string>()
	const kept = held.filter((id) => offering.has(id))
	const others = (await eligibleCores(env)).filter((core) => core.owner_id !== ownerId)
	const home = server ?? kept[0] ?? draw(others, [])
	if (!home || !members) return { home, replica: null }
	return { home, replica: kept.find((id) => id !== home) ?? draw(others, [home]) }
}

/** The Cores with these ids, in the given order, as the App reaches them. */
export async function holders(env: Env, ids: (string | null)[]): Promise<Holder[]> {
	const wanted = ids.filter((id): id is string => id !== null)
	if (!wanted.length) return []
	const rows = await env.DB.prepare(
		`SELECT ${HOLDER_COLUMNS} FROM cores WHERE id IN (SELECT value FROM json_each(?))`,
	)
		.bind(JSON.stringify(wanted))
		.all<Holder>()
	return wanted.flatMap((id) => rows.results.filter((row) => row.id === id))
}

/** The Cores that confirmed holding `root` among `ids`. */
export async function confirmedHolders(
	env: Env,
	ids: (string | null)[],
	root: string,
): Promise<Holder[]> {
	const wanted = ids.filter((id): id is string => id !== null)
	if (!wanted.length) return []
	const rows = await env.DB.prepare(
		`SELECT ${HOLDER_COLUMNS.replaceAll(', ', ', c.').replace('id', 'c.id')} FROM cores c JOIN core_holdings h ON h.core_id = c.id WHERE h.root = ? AND c.id IN (SELECT value FROM json_each(?))`,
	)
		.bind(root, JSON.stringify(wanted))
		.all<Holder>()
	return wanted.flatMap((id) => rows.results.filter((row) => row.id === id))
}

/** What a member needs to fetch a ready version: who holds it and a fetch ticket for it. */
export async function poolAccess(env: Env, holderList: Holder[], root: string) {
	return {
		holders: holderList,
		ticket: await fetchTicket(
			env,
			holderList.map((core) => core.id),
			root,
		),
	}
}

/** What the owner needs to upload a version: the home Core and a store ticket for it. */
export async function storeGrant(env: Env, homeId: string, root: string, bytes: number) {
	const [core] = await holders(env, [homeId])
	if (!core)
		throw new ApiError(503, 'no_storage', 'The Core holding this share is gone', 'no_storage')
	const { ticket, expires } = await storeTicket(env, core.id, root, bytes)
	return { core, ticket, expires }
}

/** How many ready versions `ownerId`'s shares keep. Null keeps them all. */
export async function keptVersions(env: Env, ownerId: string): Promise<number | null> {
	const row = await env.DB.prepare('SELECT kept_versions FROM sharing_settings WHERE user_id = ?')
		.bind(ownerId)
		.first<{ kept_versions: number | null }>()
	return row ? row.kept_versions : DEFAULT_KEPT_VERSIONS
}

/** The object names of a stored version, read from its manifest. */
export async function versionObjects(
	env: Env,
	instanceId: string,
	version: number,
): Promise<Map<string, number>> {
	const stored = await env.FILES.get(manifestKey(instanceId, version))
	const files = stored
		? ((await stored.json()) as { external_files: ManifestFile[] }).external_files
		: []
	return new Map(files.map((file) => [file.object, file.object_size]))
}

/** The bytes of `objects` that `before` did not have. */
export function addedBytes(objects: Map<string, number>, before: Map<string, number>): number {
	let added = 0
	for (const [object, size] of objects) if (!before.has(object)) added += size
	return added
}

/** A kept version as usage counts it, oldest first. */
export type CountedVersion = { bytes: number; added: number | null }

/**
 * A share's usage: its oldest kept version whole, plus what each later one added. A file is
 * counted once however many versions keep it, unless it left and came back.
 */
export function shareUsage(versions: CountedVersion[]): number {
	return versions.reduce(
		(sum, version, index) => sum + (index === 0 ? version.bytes : (version.added ?? version.bytes)),
		0,
	)
}

export type ShareStorage = {
	id: string
	name: string
	/** Whether it uses someone else's Core, so its usage counts against the allowance. */
	counted: boolean
	used: number
	/** Newest first. `current` is the newest ready one, which members install. */
	versions: { version: number; created: string; bytes: number; ready: boolean; current: boolean }[]
}

/**
 * Every share `ownerId` owns, with its kept versions: the ready ones, and one still uploading
 * within its store ticket's life, so pushes to several shares at once cannot each see the others
 * as empty.
 */
export async function ownerShares(env: Env, ownerId: string): Promise<ShareStorage[]> {
	const [rows, own] = await Promise.all([
		env.DB.prepare(
			`SELECT i.id, i.name, i.home, i.replica, v.version, v.bytes, v.added, v.ready, v.created
			FROM shared_instances i LEFT JOIN shared_versions v ON v.instance_id = i.id AND (v.ready = 1 OR v.created > ?2)
			WHERE i.owner_id = ?1 ORDER BY i.created, i.id, v.version`,
		)
			.bind(ownerId, new Date(Date.now() - UNCONFIRMED_TTL_MS).toISOString())
			.all<{
				id: string
				name: string
				home: string | null
				replica: string | null
				version: number | null
				bytes: number
				added: number | null
				ready: number
				created: string
			}>(),
		env.DB.prepare('SELECT id FROM cores WHERE owner_id = ?').bind(ownerId).all<{ id: string }>(),
	])
	const ownIds = new Set(own.results.map((core) => core.id))
	const shares = new Map<string, ShareStorage & { kept: CountedVersion[] }>()
	for (const row of rows.results) {
		let share = shares.get(row.id)
		if (!share) {
			share = {
				id: row.id,
				name: row.name,
				counted: [row.home, row.replica].some((id) => id !== null && !ownIds.has(id)),
				used: 0,
				versions: [],
				kept: [],
			}
			shares.set(row.id, share)
		}
		if (row.version === null) continue
		share.kept.push({ bytes: row.bytes, added: row.added })
		share.versions.unshift({
			version: row.version,
			created: row.created,
			bytes: row.bytes,
			ready: row.ready === 1,
			current: false,
		})
	}
	return [...shares.values()].map(({ kept, ...share }) => {
		const current = share.versions.find((version) => version.ready)
		if (current) current.current = true
		return { ...share, used: share.counted ? shareUsage(kept) : 0 }
	})
}
