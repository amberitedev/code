import { requireUser, tokenFrom } from '../common/auth'
import { digest } from '../common/crypto'
import { ApiError, json, readJson, routePath, stringField, unauthorized } from '../common/http'
import type { Env } from '../common/types'

// Servers visible to friends. A Core lists the servers whose "Visible to friends" setting is on,
// each with the shared instance it follows. The people that instance is shared with read the list
// and join by the Minecraft address; nobody else is given it.

const MAX_SERVERS = 100

/** SQL for "this account owns the shared instance bound as `?1` or has joined it". */
const sharedWith = (account: string) =>
	`EXISTS (SELECT 1 FROM shared_members m WHERE m.instance_id = ?1 AND m.user_id = ${account} AND m.joined_at IS NOT NULL)`

/**
 * Core, paired: replace the servers this Core shows to friends. The Core sends its whole list
 * whenever anything in it changes, so a server missing from the list stops being visible. A server
 * whose instance the Core's owner does not share is left out.
 */
async function replaceServers(request: Request, env: Env) {
	const key = tokenFrom(request)
	if (!key.startsWith('amc_')) unauthorized()
	const { servers } = await readJson(request)
	if (!Array.isArray(servers) || servers.length > MAX_SERVERS)
		throw new ApiError(400, 'invalid_input', 'Invalid servers')
	const rows = servers.map((server: unknown) => {
		if (!server || typeof server !== 'object')
			throw new ApiError(400, 'invalid_input', 'Invalid servers')
		const body: Record<string, unknown> = { ...server }
		return {
			id: stringField(body, 'id', 64),
			name: stringField(body, 'name', 100),
			address: stringField(body, 'address', 261),
			instance: stringField(body, 'shared_instance_id', 64),
		}
	})

	const core = await env.DB.prepare('SELECT id,owner_id FROM cores WHERE key_hash = ?')
		.bind(await digest(key))
		.first<{ id: string; owner_id: string }>()
	if (!core) unauthorized()
	await env.DB.batch([
		env.DB.prepare('DELETE FROM visible_servers WHERE core_id = ?').bind(core.id),
		...rows.map((row) =>
			env.DB.prepare(
				`INSERT OR REPLACE INTO visible_servers (instance_id,core_id,server_id,name,address) SELECT ?1,?3,?4,?5,?6 WHERE ${sharedWith('?2')}`,
			).bind(row.instance, core.owner_id, core.id, row.id, row.name, row.address),
		),
	])
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
	const instance = path.match(/^\/instances\/([^/]+)\/servers$/)
	if (instance && method === 'GET')
		return listServers(request, env, decodeURIComponent(instance[1]))
	return null
}
