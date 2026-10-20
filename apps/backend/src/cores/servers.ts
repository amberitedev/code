import { requireUser, tokenFrom } from '../common/auth'
import { digest } from '../common/crypto'
import { ApiError, json, readJson, routePath, stringField, unauthorized } from '../common/http'
import type { Env } from '../common/types'

// Servers visible to friends. A Core lists the servers whose "Visible to friends" setting is on,
// each with the shared instance it follows. The people that instance is shared with read the list
// and join by the Minecraft address; nobody else is given it.

const MAX_SERVERS = 100
/**
 * A server name, in UTF-16 units. Core's own limit, 64 characters (`create` in
 * `apps/core/src/api/servers.rs`), is the real one; this is loose enough for every name it allows.
 */
export const MAX_SERVER_NAME = 256

/** SQL for "this account owns the shared instance bound as `?1` or has joined it". */
const sharedWith = (account: string) =>
	`EXISTS (SELECT 1 FROM shared_members m WHERE m.instance_id = ?1 AND m.user_id = ${account} AND m.joined_at IS NOT NULL)`

/** A server a Core shows to the people its instance is shared with. */
interface VisibleServer {
	id: string
	name: string
	address: string
	instance: string
}

/** The `servers` list of a Core's report. */
export function parseServers(servers: unknown): VisibleServer[] {
	if (!Array.isArray(servers) || servers.length > MAX_SERVERS)
		throw new ApiError(400, 'invalid_input', 'Invalid servers')
	return servers.map((server: unknown) => {
		if (!server || typeof server !== 'object')
			throw new ApiError(400, 'invalid_input', 'Invalid servers')
		const body: Record<string, unknown> = { ...server }
		return {
			id: stringField(body, 'id', 64),
			name: stringField(body, 'name', MAX_SERVER_NAME),
			address: stringField(body, 'address', 261),
			instance: stringField(body, 'shared_instance_id', 64),
		}
	})
}

/**
 * Make `servers` the whole list this Core shows to friends, writing only the rows that changed. A
 * server whose instance the Core's owner does not share is left out.
 */
export async function writeServers(
	env: Env,
	core: { id: string; owner_id: string },
	servers: VisibleServer[],
) {
	const stored = await env.DB.prepare(
		'SELECT server_id AS id, name, address, instance_id AS instance FROM visible_servers WHERE core_id = ?',
	)
		.bind(core.id)
		.all<VisibleServer>()
	const before = new Map(stored.results.map((row) => [row.id, row]))
	const wanted = new Set(servers.map((server) => server.id))
	const remove = (id: string) =>
		env.DB.prepare('DELETE FROM visible_servers WHERE core_id = ? AND server_id = ?').bind(
			core.id,
			id,
		)
	const statements = [
		...stored.results.filter((row) => !wanted.has(row.id)).map((row) => remove(row.id)),
		...servers
			.filter((server) => {
				const old = before.get(server.id)
				return (
					!old ||
					old.name !== server.name ||
					old.address !== server.address ||
					old.instance !== server.instance
				)
			})
			// A changed row goes first, so one whose instance is no longer shared is not kept stale.
			.flatMap((server) => [
				remove(server.id),
				env.DB.prepare(
					`INSERT OR REPLACE INTO visible_servers (instance_id,core_id,server_id,name,address) SELECT ?1,?3,?4,?5,?6 WHERE ${sharedWith('?2')}`,
				).bind(server.instance, core.owner_id, core.id, server.id, server.name, server.address),
			]),
	]
	if (statements.length) await env.DB.batch(statements)
}

/**
 * Core on `main`, signed in: replace the servers this Core shows to friends. The Core sends its
 * whole list whenever anything in it changes, so a server missing from the list stops being
 * visible. The new Core sends the list in its state report instead.
 */
async function replaceServers(request: Request, env: Env) {
	const key = tokenFrom(request)
	if (!key.startsWith('amc_')) unauthorized()
	const servers = parseServers((await readJson(request)).servers)
	const core = await env.DB.prepare('SELECT id,owner_id FROM cores WHERE key_hash = ?')
		.bind(await digest(key))
		.first<{ id: string; owner_id: string }>()
	if (!core) unauthorized()
	await writeServers(env, core, servers)
	return new Response(null, { status: 204 })
}

/**
 * App: the servers other people host for a shared instance. Only given to the people the instance
 * is shared with. The caller's own Cores are left out; the App reads those from the Core itself.
 */
async function listServers(request: Request, env: Env, instanceId: string) {
	const user = await requireUser(request, env)
	const member = await env.DB.prepare(`SELECT ${sharedWith('?2')} AS member`)
		.bind(instanceId, user.id)
		.first<{ member: number }>()
	if (!member?.member) throw new ApiError(404, 'not_found', 'Shared instance was not found')
	const rows = await env.DB.prepare(
		`SELECT s.core_id,s.server_id,s.name,s.address,u.id AS host_id,u.username AS host_name,u.avatar_url AS host_avatar FROM visible_servers s JOIN cores c ON c.id = s.core_id JOIN users u ON u.id = c.owner_id WHERE s.instance_id = ?1 AND c.owner_id != ?2 AND ${sharedWith('c.owner_id')} ORDER BY s.name`,
	)
		.bind(instanceId, user.id)
		.all<{
			core_id: string
			server_id: string
			name: string
			address: string
			host_id: string
			host_name: string
			host_avatar: string | null
		}>()
	return json(
		rows.results.map((row) => ({
			core_id: row.core_id,
			server_id: row.server_id,
			name: row.name,
			address: row.address,
			host: { id: row.host_id, username: row.host_name, avatar_url: row.host_avatar },
		})),
	)
}

export async function handleVisibleServers(request: Request, env: Env): Promise<Response | null> {
	const path = routePath(request),
		method = request.method
	if (path === '/core/servers' && method === 'PUT') return replaceServers(request, env)
	const instance = path.match(/^\/v1\/instances\/([^/]+)\/servers$/)
	if (instance && method === 'GET')
		return listServers(request, env, decodeURIComponent(instance[1]))
	return null
}
