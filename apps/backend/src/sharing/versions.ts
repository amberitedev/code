import { ALLOWANCE_BYTES, MAX_SHARE_BYTES } from '@amberite/contracts/protocol'
import {
	type InstanceVersion,
	type NewInstanceVersion,
	type VersionFault,
	type VersionManifest,
	sharing,
} from '@amberite/contracts/backend'

import { ApiError } from '../common/http'
import { JsonText, noBody, route, spliceJson } from '../common/router'
import type { Env } from '../common/types'
import { type InstanceRow, PLAYERS, now, owner, quarantined, readable, sharer } from './access'
import { notifyReadyInvites } from './invites'
import { parseFault, parseNewVersion } from './parse'
import {
	EMPTY_ROOT,
	KEPT_VERSIONS,
	MAX_MANIFEST_BODY,
	UNCONFIRMED_TTL_MS,
	addedBytes,
	confirmedHolders,
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

// A share's versions: pushing one, reading one, and dropping old ones. Every version lists all of
// the share's files, so dropping older ones loses nothing.

export type VersionRow = {
	instance_id: string
	version: number
	version_hash: string
	bytes: number
	added: number | null
	ready: number
	created: string
}

/** The manifest of a version, read from R2 as text so the Worker never parses it. */
export async function manifestText(env: Env, item: Pick<VersionRow, 'instance_id' | 'version'>) {
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
export async function versionResponse(
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
export async function latestVersion(env: Env, id: string): Promise<VersionRow> {
	const result = await env.DB.prepare(
		'SELECT * FROM shared_instance_versions WHERE instance_id = ? ORDER BY ready DESC, version DESC LIMIT 1',
	)
		.bind(id)
		.first<VersionRow>()
	if (!result) throw new ApiError(404, 'version_not_found', 'Shared instance has no version')
	return result
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
export async function createVersion(
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

/** A version number from the path, which arrives as text. */
function versionNumber(value: number): number {
	const text = String(value)
	if (!/^\d+$/.test(text) || !Number.isSafeInteger(Number(text)))
		throw new ApiError(400, 'invalid_input', 'Invalid shared version')
	return Number(text)
}

export async function versionRow(env: Env, id: string, version: number): Promise<VersionRow> {
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
export async function memberVersion(
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

export const versionRoutes = [
	route(sharing.GetLatestVersion, noBody, async ({ request, env, params }) => {
		const user = await sharer(request, env)
		// A pending invitee reads it too: the App previews what an invite installs before accepting.
		const item = await readable(env, params.id, user.id, true)
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
]
