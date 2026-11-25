import { cores } from '@amberite/contracts/backend'

import { accountLocked } from '../common/auth'
import { ApiError } from '../common/http'
import { noBody, route } from '../common/router'
import type { Env } from '../common/types'
import { requireListedCore } from '../cores'
import { blacklisted, instance, member, now, quarantined } from './access'
import { parseNewServerShare, parseNewVersion, parseUpdate } from './parse'
import { MAX_MANIFEST_BODY } from './pool'
import { deleteShare, renameShare } from './shares'
import { createVersion, latestVersion, versionResponse, versionRow } from './versions'

// Sharing as a Core reaches it with its own key, on its own routes, acting for its owner.

/**
 * The calling Core. Its owner's lock and sharing ban come with its key's lookup, so they cost no
 * further read.
 */
async function sharingCore(request: Request, env: Env, alwaysAllowed = false) {
	const core = await requireListedCore(request, env)
	// An owner may always delete their share, locked or not.
	if (!alwaysAllowed) {
		if (core.owner_locked) accountLocked()
		if (core.owner_blacklisted) throw blacklisted()
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
 * A version a Core reads for its owner: of its own server's share, or of a share a server follows.
 * The owner must read the share, so a Core reads nothing its owner cannot. Its owner's lock and
 * sharing ban come with its key.
 */
async function coreVersion(request: Request, env: Env, id: string, version?: number) {
	const core = await sharingCore(request, env)
	const item = await instance(env, id)
	await member(env, item, core.owner_id)
	if (item.quarantine) throw quarantined()
	const row =
		version === undefined ? await latestVersion(env, id) : await versionRow(env, id, version)
	return versionResponse(env, item, row, null)
}

/**
 * A Core, with its key, writing the share of one of its server-first servers (a server that
 * follows no instance) as the share's owner, its own owner account: create (answering the
 * server's existing share if it has one), rename, delete, and its versions. It also reads the
 * versions of any share its owner reads, which a server may follow (`content::shared` in Core).
 * It reaches no other share.
 */
export const coreKeyRoutes = [
	route(cores.CreateServerShare, parseNewServerShare, async ({ request, env, body }) => {
		const core = await sharingCore(request, env)
		const { name, server_id } = await body()
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
	route(cores.RenameServerShare, parseUpdate, async ({ request, env, params, body }) => {
		await coreShare(env, await sharingCore(request, env), params.id)
		await renameShare(env, params.id, await body())
	}),
	route(cores.DeleteServerShare, noBody, async ({ request, env, params }) => {
		await coreShare(env, await sharingCore(request, env, true), params.id)
		await deleteShare(env, params.id)
	}),
	route(
		cores.CreateServerShareVersion,
		parseNewVersion,
		async ({ request, env, params, body }) => {
			const core = await sharingCore(request, env)
			const item = await coreShare(env, core, params.id)
			if (item.quarantine) throw quarantined()
			return createVersion(env, item, core.owner_id, await body(), core.id)
		},
		MAX_MANIFEST_BODY,
	),
	route(cores.GetLatestShareVersion, noBody, async ({ request, env, params }) =>
		coreVersion(request, env, params.id),
	),
	route(cores.GetShareVersion, noBody, async ({ request, env, params }) =>
		coreVersion(request, env, params.id, params.version),
	),
]
