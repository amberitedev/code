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
/** The sum of an account's shares' latest versions. */
export const MAX_USER_BYTES = 3 * 1024 ** 3
/** How long an unconfirmed version blocks the next push: the store ticket's life. */
export const UNCONFIRMED_TTL_MS = 60 * 60_000
/** A Core that has not reported its storage for this long takes no new shares. */
const REPORT_FRESH_MS = 2 * 86_400_000
/** R of a version with no files: SHA-256 of nothing. */
export const EMPTY_ROOT = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
/** Files encrypted with a key derived from their own content, so one copy serves every share. */
const CONTENT_TYPES = new Set(['mod', 'resourcepack', 'shader', 'datapack'])
const HASH = /^[0-9a-f]{64}$/

export const manifestKey = (instanceId: string, version: number) =>
	`manifests/${instanceId}/${version}`

export type ManifestFile = {
	file_name: string
	file_type: string
	/** `pool:<C>`, for upstream code that expects a URL; nothing fetches it. */
	url: string
	size: number
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
	const hash = (field: string) => {
		const value = entry[field]
		if (typeof value !== 'string' || !HASH.test(value))
			throw new ApiError(400, 'invalid_input', `Invalid ${field} for ${file_name}`)
		return value
	}
	const size = count('size')
	const object_size = count('object_size')
	// Every chunk of 1 MiB, and the last one even when empty, carries a 16-byte tag.
	if (object_size !== size + 16 * Math.max(1, Math.ceil(size / 1_048_576)))
		throw new ApiError(400, 'invalid_input', `Invalid object_size for ${file_name}`)
	const object = hash('object')
	const file: ManifestFile = {
		file_name,
		file_type,
		url: `pool:${object}`,
		size,
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
 * Cores that may take a new share: storage on, running as a service with a public address (not
 * required in local development), room under their cap, and a storage report within two days.
 */
async function eligibleCores(env: Env): Promise<Candidate[]> {
	const local = env.LOCAL_DEV === 'true'
	const rows = await env.DB.prepare(
		`SELECT c.${HOLDER_COLUMNS.replaceAll(', ', ', c.')}, c.owner_id, s.cap - s.used AS room FROM cores c JOIN core_storage s ON s.core_id = c.id WHERE s.enabled = 1 AND s.cap > s.used AND s.reported > ?1 AND (?2 OR (s.service = 1 AND c.public_address IS NOT NULL)) AND c.cert_fingerprint IS NOT NULL`,
	)
		.bind(new Date(Date.now() - REPORT_FRESH_MS).toISOString(), local ? 1 : 0)
		.all<Candidate>()
	return rows.results
}

/**
 * The Core a new share goes to: the owner's own if eligible, else one drawn by free space, so a
 * new large disk does not take every share until it is the fullest. Null when none is eligible.
 */
export async function pickHome(env: Env, ownerId: string): Promise<string | null> {
	const cores = await eligibleCores(env)
	const own = cores.find((core) => core.owner_id === ownerId)
	if (own) return own.id
	const total = cores.reduce((sum, core) => sum + core.room, 0)
	if (!total) return null
	let draw = Math.random() * total
	for (const core of cores) {
		draw -= core.room
		if (draw < 0) return core.id
	}
	return cores[cores.length - 1]?.id ?? null
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

/**
 * What the owner's other shares hold, so the new version keeps the account under its quota: the
 * sum of every share's latest version, minus this share's, which the new version replaces.
 */
export async function otherSharesBytes(
	env: Env,
	ownerId: string,
	instanceId: string,
): Promise<number> {
	const row = await env.DB.prepare(
		'SELECT COALESCE(SUM(v.bytes),0) AS total FROM shared_instances i JOIN shared_versions v ON v.instance_id = i.id AND v.version = (SELECT MAX(version) FROM shared_versions WHERE instance_id = i.id AND ready = 1) WHERE i.owner_id = ? AND i.id != ?',
	)
		.bind(ownerId, instanceId)
		.first<{ total: number }>()
	return row?.total ?? 0
}
