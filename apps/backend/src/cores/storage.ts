import { ApiError, json, readBytes } from '../common/http'
import type { Env } from '../common/types'
import { UNCONFIRMED_TTL_MS, holders, place } from '../sharing/pool'
import { versionReady } from '../sharing'
import { pullTicket } from './tickets'

// A Core's storage report: what its store holds and how much room it has, sent after every
// completed upload or pull and once an hour. The backend confirms the versions whose home it is,
// tells it which versions to drop and which to pull, and writes only what changed. See
// docs/internals/pool.md, "Core and backend".

const MAX_VERSIONS = 10_000
/** A report of `MAX_VERSIONS` versions is about a megabyte. */
const MAX_REPORT_BODY = 2 * 1024 * 1024
/** Pull orders per report, so one report cannot ask a Core to copy everything at once. */
const MAX_PULLS = 20
/** A report that changed nothing still refreshes `reported` once it is this old. */
const REFRESH_MS = 12 * 3_600_000
const HASH = /^[0-9a-f]{64}$/

interface StorageRow {
	core_id: string
	enabled: number
	service: number
	cap: number
	used: number
	free: number
	egress_cap: number
	egress_today: number
	reported: string
}

const STORAGE_FIELDS = [
	'enabled',
	'service',
	'cap',
	'used',
	'free',
	'egress_cap',
	'egress_today',
] as const

function natural(body: Record<string, unknown>, field: string): number {
	const value = body[field]
	if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0)
		throw new ApiError(400, 'invalid_input', `Invalid ${field}`)
	return value
}

function parseVersions(value: unknown): Map<string, number> {
	if (!Array.isArray(value) || value.length > MAX_VERSIONS)
		throw new ApiError(400, 'invalid_input', 'Invalid versions')
	const versions = new Map<string, number>()
	for (const entry of value) {
		if (!entry || typeof entry !== 'object')
			throw new ApiError(400, 'invalid_input', 'Invalid versions')
		const body: Record<string, unknown> = { ...entry }
		if (typeof body.root !== 'string' || !HASH.test(body.root))
			throw new ApiError(400, 'invalid_input', 'Invalid root')
		versions.set(body.root, natural(body, 'bytes'))
	}
	return versions
}

/** Core, signed in: its storage now. */
export async function reportStorage(
	request: Request,
	env: Env,
	core: { id: string; owner_id: string },
) {
	let body: unknown
	try {
		body = JSON.parse(new TextDecoder().decode(await readBytes(request, MAX_REPORT_BODY)))
	} catch {
		throw new ApiError(400, 'invalid_input', 'Invalid JSON')
	}
	if (!body || typeof body !== 'object' || Array.isArray(body))
		throw new ApiError(400, 'invalid_input', 'Expected a JSON object')
	const report = body as Record<string, unknown>
	const next = {
		enabled: report.enabled === true ? 1 : 0,
		service: report.service === true ? 1 : 0,
		cap: natural(report, 'cap'),
		used: natural(report, 'used'),
		free: natural(report, 'free'),
		egress_cap: natural(report, 'egress_cap'),
		egress_today: natural(report, 'egress_today'),
	}
	const versions = parseVersions(report.versions)
	const nowText = new Date().toISOString()

	const stored = await env.DB.prepare('SELECT * FROM core_storage WHERE core_id = ?')
		.bind(core.id)
		.first<StorageRow>()
	const changed =
		!stored ||
		STORAGE_FIELDS.some((field) => stored[field] !== next[field]) ||
		Date.parse(stored.reported) < Date.now() - REFRESH_MS
	const held = (
		await env.DB.prepare('SELECT root, bytes FROM core_holdings WHERE core_id = ?')
			.bind(core.id)
			.all<{ root: string; bytes: number }>()
	).results
	const before = new Map(held.map((row) => [row.root, row.bytes]))
	const statements = [
		...(changed
			? [
					env.DB.prepare(
						'INSERT OR REPLACE INTO core_storage (core_id,enabled,service,cap,used,free,egress_cap,egress_today,reported) VALUES (?,?,?,?,?,?,?,?,?)',
					).bind(
						core.id,
						next.enabled,
						next.service,
						next.cap,
						next.used,
						next.free,
						next.egress_cap,
						next.egress_today,
						nowText,
					),
				]
			: []),
		...held
			.filter((row) => !versions.has(row.root))
			.map((row) =>
				env.DB.prepare('DELETE FROM core_holdings WHERE core_id = ? AND root = ?').bind(
					core.id,
					row.root,
				),
			),
		...[...versions]
			.filter(([root, bytes]) => before.get(root) !== bytes)
			.map(([root, bytes]) =>
				env.DB.prepare(
					'INSERT OR REPLACE INTO core_holdings (core_id,root,bytes,confirmed) VALUES (?,?,?,?)',
				).bind(core.id, root, bytes, nowText),
			),
	]
	if (statements.length) await env.DB.batch(statements)

	// Versions whose home this Core is and whose objects it now holds are ready.
	const newRoots = [...versions.keys()].filter((root) => !before.has(root))
	if (newRoots.length) {
		const confirmed = (
			await env.DB.prepare(
				'SELECT v.instance_id, v.version FROM shared_versions v JOIN shared_instances i ON i.id = v.instance_id WHERE v.ready = 0 AND i.home = ?1 AND v.root IN (SELECT value FROM json_each(?2))',
			)
				.bind(core.id, JSON.stringify(newRoots))
				.all<{ instance_id: string; version: number }>()
		).results
		for (const row of confirmed) await versionReady(env, row.instance_id, row.version)
	}

	if (!next.enabled) await handOn(env, core)

	return json({ drop: await dropList(env, core.id), pull: await pullList(env, core.id) })
}

/**
 * A Core with storage off runs privately, so other people's shares on it move to Cores that
 * offer storage, and it drops them once they are gone from it. Only a share with another holder
 * moves now: the new Core copies from that one. A share held here alone stays until its owner's
 * next push puts it somewhere else.
 */
async function handOn(env: Env, core: { id: string; owner_id: string }) {
	const shares = (
		await env.DB.prepare(
			`SELECT i.id, i.owner_id, i.home, i.replica,
				(EXISTS (SELECT 1 FROM shared_members m WHERE m.instance_id = i.id AND m.join_type != 'owner')
					OR EXISTS (SELECT 1 FROM server_players p JOIN shared_servers s USING (core_id, server_id) WHERE s.instance_id = i.id)) AS members
			FROM shared_instances i WHERE (i.home = ?1 OR i.replica = ?1) AND i.owner_id != ?2
			AND i.home IS NOT NULL AND i.replica IS NOT NULL`,
		)
			.bind(core.id, core.owner_id)
			.all<{ id: string; owner_id: string; home: string; replica: string; members: number }>()
	).results
	for (const share of shares) {
		const placement = await place(env, share.owner_id, share, {
			serverCore: share.home,
			members: share.members === 1,
		})
		if (!placement.home) continue
		await env.DB.prepare('UPDATE shared_instances SET home = ?, replica = ? WHERE id = ?')
			.bind(placement.home, placement.replica, share.id)
			.run()
	}
}

/**
 * Roots this Core holds that no share on it needs: not a kept version (ready, or still being
 * uploaded) of a share homed or replicated here, and not the newest version this Core itself
 * confirmed for such a share, so a replica keeps an old version until it holds a newer one.
 */
async function dropList(env: Env, coreId: string): Promise<string[]> {
	const fresh = new Date(Date.now() - UNCONFIRMED_TTL_MS).toISOString()
	const rows = await env.DB.prepare(
		`SELECT h.root FROM core_holdings h WHERE h.core_id = ?1
		AND h.root NOT IN (
			SELECT v.root FROM shared_versions v JOIN shared_instances i ON i.id = v.instance_id
			WHERE (i.home = ?1 OR i.replica = ?1) AND (v.ready = 1 OR v.created > ?2))
		AND h.root NOT IN (
			SELECT v.root FROM shared_versions v JOIN shared_instances i ON i.id = v.instance_id
			WHERE (i.home = ?1 OR i.replica = ?1)
			AND v.version = (SELECT MAX(v2.version) FROM shared_versions v2 JOIN core_holdings h2 ON h2.core_id = ?1 AND h2.root = v2.root WHERE v2.instance_id = i.id))`,
	)
		.bind(coreId, fresh)
		.all<{ root: string }>()
	return rows.results.map((row) => row.root)
}

/**
 * Versions this Core should hold but does not, each with a pull ticket for the other holder that
 * has it: every kept ready version of every share homed or replicated here, newest first.
 */
async function pullList(env: Env, coreId: string) {
	const rows = await env.DB.prepare(
		`SELECT DISTINCT v.root, CASE WHEN i.home = ?1 THEN i.replica ELSE i.home END AS source
		FROM shared_versions v JOIN shared_instances i ON i.id = v.instance_id
		WHERE (i.home = ?1 OR i.replica = ?1) AND v.ready = 1
		AND v.root NOT IN (SELECT root FROM core_holdings WHERE core_id = ?1)
		AND EXISTS (SELECT 1 FROM core_holdings s WHERE s.root = v.root AND s.core_id = CASE WHEN i.home = ?1 THEN i.replica ELSE i.home END)
		ORDER BY v.created DESC LIMIT ?2`,
	)
		.bind(coreId, MAX_PULLS)
		.all<{ root: string; source: string }>()
	const sources = await holders(env, [...new Set(rows.results.map((row) => row.source))])
	const orders = []
	for (const row of rows.results) {
		const source = sources.find((core) => core.id === row.source)
		if (source)
			orders.push({ root: row.root, ticket: await pullTicket(env, coreId, row.root, source) })
	}
	return orders
}
