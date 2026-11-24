import { READ_TICKET_HEADER, sharing } from '@amberite/contracts'

import { accountLocked } from '../common/auth'
import { ApiError } from '../common/http'
import { noBody, route } from '../common/router'
import type { Env } from '../common/types'
import { requireListedCore } from '../cores'
import { readTicket, readTicketUser } from '../cores/tickets'
import {
	blacklisted,
	instance,
	member,
	now,
	quarantined,
	readable,
	requireSharingAllowed,
	sharer,
} from './access'
import { object, parseNewShare, parseNewVersion, parseUpdate, string } from './parse'
import { MAX_MANIFEST_BODY } from './pool'
import { deleteShare, renameShare } from './shares'
import { createVersion, latestVersion, versionResponse, versionRow } from './versions'

// Sharing as a Core reaches it with its own key, and the read tickets that let it read a share
// for a user.

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
		// The Core's owner was checked with its key.
		if (reader !== core.owner_id) await requireSharingAllowed(env, reader)
		await member(env, item, reader)
	}
	if (item.quarantine) throw quarantined()
	const row =
		version === undefined ? await latestVersion(env, id) : await versionRow(env, id, version)
	return versionResponse(env, item, row, null)
}

/**
 * A Core, with its key, writing the share of one of its server-first servers (a server that
 * follows no instance). It acts as the share's owner, its own owner account, for: create (with
 * `server_id`, answering the server's existing share if it has one), rename, delete, and its
 * versions. A Core also reads the versions of a share one of its servers follows, for a user who
 * can read it and gave it a read ticket (`content::shared` in Core). It reaches no other share and
 * no other route.
 */
export const coreKeyRoutes = [
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

export const readTicketRoutes = [
	// A ticket for one Core to read this share for the user, so a server on it can follow the share.
	// The Core sends it with its reads, and the backend checks the user can still read then.
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
]
