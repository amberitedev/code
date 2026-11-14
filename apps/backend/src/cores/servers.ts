import { sharing, type VisibleServer } from '@amberite/contracts'

import { requireUser } from '../common/auth'
import { ApiError } from '../common/http'
import { notifyMembers } from '../common/notifications'
import { noBody, route } from '../common/router'
import type { Env } from '../common/types'

// Servers visible to friends. A Core lists the servers whose "Visible to friends" setting is on,
// each with the shared instance it follows. The people that instance is shared with read the list
// and join by the Minecraft address; nobody else is given it.

/**
 * A server name, in UTF-16 units. Core's own limit, 64 characters (`create` in
 * `apps/core/src/api/servers.rs`), is the real one; this is loose enough for every name it allows.
 */
export const MAX_SERVER_NAME = 256

/** SQL for "this account owns the shared instance bound as `?1` or has joined it". */
const sharedWith = (account: string) =>
	`EXISTS (SELECT 1 FROM shared_instance_users m WHERE m.instance_id = ?1 AND m.user_id = ${account} AND m.joined_at IS NOT NULL)`

/**
 * Make `servers` the whole list this Core shows to friends, writing only the rows that changed. A
 * server whose instance the Core's owner does not share is left out. The members of every instance
 * whose list changed are told, so their App fetches it again.
 */
export async function writeServers(
	env: Env,
	core: { id: string; owner_id: string },
	servers: VisibleServer[],
) {
	const stored = await env.DB.prepare(
		'SELECT server_id AS id, name, address, instance_id AS shared_instance_id FROM visible_servers WHERE core_id = ?',
	)
		.bind(core.id)
		.all<VisibleServer>()
	const before = new Map(stored.results.map((row) => [row.id, row]))
	const wanted = new Set(servers.map((server) => server.id))
	const removed = stored.results.filter((row) => !wanted.has(row.id))
	const changed = servers.filter((server) => {
		const old = before.get(server.id)
		return (
			!old ||
			old.name !== server.name ||
			old.address !== server.address ||
			old.shared_instance_id !== server.shared_instance_id
		)
	})
	const remove = (id: string) =>
		env.DB.prepare('DELETE FROM visible_servers WHERE core_id = ? AND server_id = ?').bind(
			core.id,
			id,
		)
	const statements = [
		...removed.map((row) => remove(row.id)),
		// A changed row goes first, so one whose instance is no longer shared is not kept stale.
		...changed.flatMap((server) => [
			remove(server.id),
			env.DB.prepare(
				`INSERT OR REPLACE INTO visible_servers (instance_id,core_id,server_id,name,address) SELECT ?1,?3,?4,?5,?6 WHERE ${sharedWith('?2')}`,
			).bind(
				server.shared_instance_id,
				core.owner_id,
				core.id,
				server.id,
				server.name,
				server.address,
			),
		]),
	]
	if (!statements.length) return
	await env.DB.batch(statements)
	const touched = [...removed, ...changed, ...changed.map((server) => before.get(server.id))]
	const instances = new Set(touched.flatMap((row) => (row ? [row.shared_instance_id] : [])))
	await Promise.all(
		[...instances].map((id) => notifyMembers(env, id, 'shared_instance_servers', core.owner_id)),
	)
}

/**
 * App: the servers other people host for a shared instance. Only given to the people the instance
 * is shared with. The caller's own Cores are left out; the App reads those from the Core itself.
 */
export const listHostedServers = route(
	sharing.ListHostedServers,
	noBody,
	async ({ request, env, params }) => {
		const user = await requireUser(request, env)
		const member = await env.DB.prepare(`SELECT ${sharedWith('?2')} AS member`)
			.bind(params.id, user.id)
			.first<{ member: number }>()
		if (!member?.member) throw new ApiError(404, 'not_found', 'Shared instance was not found')
		const rows = await env.DB.prepare(
			`SELECT s.core_id,s.server_id,s.name,s.address,u.id AS host_id,u.username AS host_name,u.avatar_url AS host_avatar FROM visible_servers s JOIN cores c ON c.id = s.core_id JOIN users u ON u.id = c.owner_id WHERE s.instance_id = ?1 AND c.owner_id != ?2 AND ${sharedWith('c.owner_id')} ORDER BY s.name`,
		)
			.bind(params.id, user.id)
			.all<{
				core_id: string
				server_id: string
				name: string
				address: string
				host_id: string
				host_name: string
				host_avatar: string | null
			}>()
		return rows.results.map((row) => ({
			core_id: row.core_id,
			server_id: row.server_id,
			name: row.name,
			address: row.address,
			host: { id: row.host_id, username: row.host_name, avatar_url: row.host_avatar },
		}))
	},
)
