import type { IncomingMessage, ServerResponse } from 'node:http'

import type { Archon } from '../../api-client/src/modules/archon/types.ts'

import { PERSONAS, SERVER_ID, WORLD_ID } from './fixtures.ts'
import { empty, HttpError, json, objectBody, readJson, stringField } from './http.ts'
import type { StateStore } from './state.ts'
import type { Persona } from './types.ts'
import type { WebSocketHub } from './websocket.ts'

type HostingContext = {
	request: IncomingMessage
	response: ServerResponse
	url: URL
	persona: Persona | null
	store: StateStore
	sockets: WebSocketHub
}

export async function handleHosting(context: HostingContext): Promise<boolean> {
	const { request, response, url, persona, store, sockets } = context
	const method = request.method ?? 'GET'
	const path = url.pathname

	if (method === 'GET' && path === '/v1/regions') {
		json(response, 200, [
			{
				shortcode: 'local',
				country_code: 'ZZ',
				display_name: 'Local proxy lab',
				lat: 0,
				lon: 0,
				zone: 'local',
			},
		] satisfies Archon.Servers.v1.Region[])
		return true
	}

	if (
		method === 'POST' &&
		(path === '/modrinth/v0/stock' || path.startsWith('/modrinth/v0/stock?'))
	) {
		json(response, 200, { available: 1 } satisfies Archon.Servers.v0.StockResponse)
		return true
	}

	if (!path.startsWith('/modrinth/v0/') && !path.startsWith('/v1/')) return false
	requirePersona(persona)

	const state = store.read()

	if (method === 'GET' && path === '/modrinth/v0/servers') {
		json(response, 200, {
			servers: [state.archon.server_v0],
			pagination: { current_page: 1, page_size: 100, total_pages: 1, total_items: 1 },
			users: {
				[PERSONAS.owner.user.id]: {
					id: PERSONAS.owner.user.id,
					username: PERSONAS.owner.user.username,
					avatar_url: null,
				},
			},
		} satisfies Archon.Servers.v0.ServerGetResponse)
		return true
	}

	if (method === 'GET' && path === `/modrinth/v0/servers/${SERVER_ID}`) {
		json(response, 200, state.archon.server_v0)
		return true
	}

	if (method === 'GET' && path === `/modrinth/v0/servers/${SERVER_ID}/ws`) {
		json(response, 200, {
			url: `${url.origin.replace('http:', 'ws:')}/ws/${SERVER_ID}`,
			token: 'proxy-lab-websocket-token',
		} satisfies Archon.Websocket.v0.WSAuth)
		return true
	}

	if (method === 'GET' && path === `/modrinth/v0/servers/${SERVER_ID}/fs`) {
		json(response, 200, {
			url: `${url.origin}/node/modrinth/v0/fs`,
			token: 'proxy-lab-node-token',
		} satisfies Archon.Servers.v0.JWTAuth)
		return true
	}

	if (method === 'POST' && path === `/modrinth/v0/servers/${SERVER_ID}/power`) {
		const body = objectBody(await readJson(request))
		const action = stringField(body, 'action')
		if (!['Start', 'Stop', 'Restart', 'Kill'].includes(action)) {
			throw new HttpError(400, `Unsupported power action "${action}"`)
		}
		const powerState: Archon.Websocket.v0.PowerState =
			action === 'Start' || action === 'Restart' ? 'running' : 'stopped'
		await store.mutate((current) => {
			current.archon.power_state = powerState
			current.archon.started_at = powerState === 'running' ? new Date().toISOString() : null
			appendAction(
				current,
				{
					Start: 'server_started',
					Stop: 'server_stopped',
					Restart: 'server_restarted',
					Kill: 'server_killed',
				}[action]!,
			)
		})
		sockets.broadcastHosting({ event: 'power-state', state: powerState })
		sockets.broadcastHosting({
			event: 'log',
			stream: 'stdout',
			message: `[Proxy Lab] ${action} completed. No Minecraft process was started.`,
		})
		empty(response)
		return true
	}

	if (method === 'POST' && path === `/modrinth/v0/servers/${SERVER_ID}/name`) {
		const name = stringField(objectBody(await readJson(request)), 'name')
		await store.mutate((current) => {
			current.archon.server_v0.name = name
			current.archon.server_v1.name = name
			appendAction(current, 'changed_server_name')
		})
		empty(response)
		return true
	}

	if (method === 'POST' && path === `/modrinth/v0/servers/${SERVER_ID}/subdomain`) {
		const subdomain = stringField(objectBody(await readJson(request)), 'subdomain')
		await store.mutate((current) => {
			current.archon.server_v1.subdomain = subdomain
			current.archon.server_v0.net.domain = `${subdomain}.proxy-lab.invalid`
			appendAction(current, 'changed_server_subdomain')
		})
		empty(response)
		return true
	}

	const subdomainMatch = path.match(/^\/modrinth\/v0\/subdomains\/([^/]+)\/isavailable$/)
	if (method === 'GET' && subdomainMatch) {
		json(response, 200, { available: subdomainMatch[1] !== state.archon.server_v1.subdomain })
		return true
	}

	if (method === 'GET' && path === `/modrinth/v0/servers/${SERVER_ID}/allocations`) {
		json(response, 200, state.archon.allocations)
		return true
	}

	if (method === 'POST' && path === `/modrinth/v0/servers/${SERVER_ID}/allocations`) {
		const name = url.searchParams.get('name')?.trim()
		if (!name) throw new HttpError(400, 'Allocation name is required')
		const allocation = await store.mutate((current) => {
			const nextPort = Math.max(...current.archon.allocations.map((item) => item.port)) + 1
			const created = { port: nextPort, name }
			current.archon.allocations.push(created)
			appendAction(current, 'port_allocation_added')
			return created
		})
		json(response, 200, allocation)
		return true
	}

	const allocationMatch = path.match(
		new RegExp(`^/modrinth/v0/servers/${SERVER_ID}/allocations/(\\d+)$`),
	)
	if (allocationMatch && method === 'PUT') {
		const port = Number(allocationMatch[1])
		const name = url.searchParams.get('name')?.trim()
		if (!name) throw new HttpError(400, 'Allocation name is required')
		await store.mutate((current) => {
			const allocation = current.archon.allocations.find((item) => item.port === port)
			if (!allocation) throw new HttpError(404, `Allocation ${port} was not found`)
			allocation.name = name
		})
		empty(response)
		return true
	}
	if (allocationMatch && method === 'DELETE') {
		const port = Number(allocationMatch[1])
		await store.mutate((current) => {
			current.archon.allocations = current.archon.allocations.filter((item) => item.port !== port)
			appendAction(current, 'port_allocation_removed')
		})
		empty(response)
		return true
	}

	if (
		method === 'POST' &&
		path.match(new RegExp(`^/modrinth/v0/servers/${SERVER_ID}/notices/\\d+/dismiss$`))
	) {
		const noticeId = Number(path.split('/').at(-2))
		await store.mutate((current) => {
			current.archon.server_v0.notices = current.archon.server_v0.notices.filter(
				(notice) => notice.id !== noticeId,
			)
		})
		empty(response)
		return true
	}

	if (method === 'GET' && path === '/v1/servers') {
		json(response, 200, [state.archon.server_v1])
		return true
	}
	if (method === 'GET' && path === `/v1/servers/${SERVER_ID}`) {
		json(response, 200, state.archon.server_v1)
		return true
	}
	if (method === 'DELETE' && path === `/v1/servers/${SERVER_ID}/flows/intro`) {
		await store.mutate((current) => {
			current.archon.server_v0.flows.intro = false
		})
		empty(response)
		return true
	}
	if (method === 'POST' && path === `/v1/servers/${SERVER_ID}/sftp/roll`) {
		const credentials = await store.mutate((current) => {
			current.archon.server_v1.sftp_password = `fixture-${Date.now()}`
			current.archon.server_v0.sftp_password = current.archon.server_v1.sftp_password
			return {
				sftp_username: current.archon.server_v1.sftp_username,
				sftp_password: current.archon.server_v1.sftp_password,
			}
		})
		json(response, 200, credentials)
		return true
	}

	if (method === 'GET' && path === '/v1/sync') {
		openSyncStream(response, state)
		return true
	}

	if (method === 'GET' && path === `/v1/servers/${SERVER_ID}/action-log`) {
		json(response, 200, {
			next_offset: null,
			data: [...state.archon.actions].reverse(),
			users: {
				[OWNER_USER_ID]: { username: PERSONAS.owner.user.username, avatar_url: null },
				[RECIPIENT_USER_ID]: {
					username: PERSONAS.recipient.user.username,
					avatar_url: null,
				},
			},
			addons: {},
			versions: {},
		} satisfies Archon.Actions.v1.ActionLogResponse)
		return true
	}

	if (await handleUsers(context)) return true
	if (await handleBackups(context)) return true
	if (await handleContent(context)) return true

	if (method === 'GET' && path === `/v1/servers/${SERVER_ID}/worlds/${WORLD_ID}/properties`) {
		json(response, 200, state.archon.properties)
		return true
	}
	if (method === 'PATCH' && path === `/v1/servers/${SERVER_ID}/worlds/${WORLD_ID}/properties`) {
		const body = objectBody(await readJson(request)) as Archon.Content.v1.PatchPropertiesFields
		const properties = await store.mutate((current) => {
			current.archon.properties.known = {
				...current.archon.properties.known,
				...body.known,
			}
			for (const [key, value] of Object.entries(body.custom ?? {})) {
				if (value === null) delete current.archon.properties.custom?.[key]
				else (current.archon.properties.custom ??= {})[key] = value
			}
			appendAction(current, 'server_properties_modified')
			return current.archon.properties
		})
		json(response, 200, properties)
		return true
	}

	if (method === 'GET' && path === `/v1/servers/${SERVER_ID}/worlds/${WORLD_ID}/options/startup`) {
		json(response, 200, state.archon.startup)
		return true
	}
	if (
		method === 'PATCH' &&
		path === `/v1/servers/${SERVER_ID}/worlds/${WORLD_ID}/options/startup`
	) {
		const body = objectBody(await readJson(request)) as Archon.Content.v1.PatchRuntimeOptions
		await store.mutate((current) => {
			current.archon.startup = { ...current.archon.startup, ...body }
			appendAction(current, 'startup_command_modified')
		})
		empty(response)
		return true
	}

	return false
}

async function handleUsers(context: HostingContext): Promise<boolean> {
	const { request, response, url, store } = context
	const method = request.method ?? 'GET'
	const collectionPath = `/v1/servers/${SERVER_ID}/users`
	if (url.pathname === collectionPath && method === 'GET') {
		json(response, 200, store.read().archon.users)
		return true
	}
	if (url.pathname === collectionPath && method === 'POST') {
		const body = objectBody(await readJson(request))
		const userId = stringField(body, 'user_id')
		const role = stringField(body, 'role')
		await store.mutate((state) => {
			if (!state.archon.users.some((item) => item.user.id === userId)) {
				state.archon.users.push({
					user: { id: userId, username: userId, avatar_url: null },
					added_on: new Date().toISOString(),
					last_invite_sent: new Date().toISOString(),
					permissions: role === 'Editor' ? 2 : 1,
				})
			}
			appendAction(state, 'user_invited')
		})
		empty(response)
		return true
	}

	const match = url.pathname.match(new RegExp(`^${collectionPath}/([^/]+)(/reinvite)?$`))
	if (!match) return false
	const userId = decodeURIComponent(match[1]!)
	if (method === 'POST' && match[2] === '/reinvite') {
		json(response, 200, { sent: true, cooldown_seconds: null })
		return true
	}
	if (method === 'DELETE') {
		await store.mutate((state) => {
			state.archon.users = state.archon.users.filter((item) => item.user.id !== userId)
			appendAction(state, 'user_removed')
		})
		empty(response)
		return true
	}
	if (method === 'PATCH') {
		const rawBody = await readJson(request)
		const role = typeof rawBody === 'string' ? rawBody : String(rawBody)
		await store.mutate((state) => {
			const user = state.archon.users.find((item) => item.user.id === userId)
			if (!user) throw new HttpError(404, `Server user ${userId} was not found`)
			user.permissions = role === 'Editor' ? 2 : 1
			appendAction(state, 'user_permission_modified')
		})
		empty(response)
		return true
	}
	return false
}

async function handleBackups(context: HostingContext): Promise<boolean> {
	const { request, response, url, store, persona } = context
	const method = request.method ?? 'GET'
	const base = `/v1/servers/${SERVER_ID}/worlds/${WORLD_ID}`
	const legacy = `${base}/backups`
	const queue = `${base}/backups-queue`
	const state = store.read()

	if (method === 'GET' && url.pathname === legacy) {
		json(response, 200, state.archon.backups)
		return true
	}
	if (method === 'GET' && url.pathname === queue) {
		json(response, 200, state.archon.backup_queue)
		return true
	}
	if (method === 'POST' && (url.pathname === queue || url.pathname === legacy)) {
		const name = stringField(objectBody(await readJson(request)), 'name')
		const id = await store.mutate((current) => {
			const id = `proxy-lab-backup-${current.next_backup++}`
			const createdAt = new Date().toISOString()
			current.archon.backups.push({
				id,
				physical_id: id,
				name,
				created_at: createdAt,
				automated: false,
				status: 'done',
				interrupted: false,
				ongoing: false,
				locked: false,
			})
			current.archon.backup_queue.backups.push({
				id,
				name,
				created_at: createdAt,
				status: 'done',
				locked: false,
				automated: false,
				history: [
					{
						operation_type: 'create',
						operation_id: current.next_backup,
						state: 'completed',
						scheduled_for: createdAt,
						started_at: createdAt,
						completed_at: createdAt,
						has_parent: false,
						error: null,
						should_prompt: false,
						synthetic_legacy: false,
						user_info: persona
							? { id: persona.user.id, username: persona.user.username, avatar_url: null }
							: null,
					},
				],
			})
			appendAction(current, 'backup_created')
			return id
		})
		json(response, 200, { id })
		return true
	}

	if (method === 'POST' && url.pathname === `${queue}/delete-many`) {
		const body = objectBody(await readJson(request))
		const ids = Array.isArray(body.backup_ids)
			? body.backup_ids.filter((id): id is string => typeof id === 'string')
			: []
		await deleteBackups(store, ids)
		empty(response)
		return true
	}

	const legacyMatch = url.pathname.match(new RegExp(`^${legacy}/([^/]+)(/restore|/retry)?$`))
	const queueMatch = url.pathname.match(new RegExp(`^${queue}/([^/]+)(/restore|/retry)?$`))
	const match = legacyMatch ?? queueMatch
	if (!match) {
		if (url.pathname.startsWith(`${queue}/history/`) && method === 'POST') {
			empty(response)
			return true
		}
		return false
	}
	const backupId = decodeURIComponent(match[1]!)
	if (method === 'GET' && legacyMatch) {
		const backup = state.archon.backups.find((item) => item.id === backupId)
		if (!backup) throw new HttpError(404, `Backup ${backupId} was not found`)
		json(response, 200, backup)
		return true
	}
	if (method === 'DELETE') {
		await deleteBackups(store, [backupId])
		empty(response)
		return true
	}
	if (method === 'PATCH' && legacyMatch) {
		const body = objectBody(await readJson(request))
		const name = stringField(body, 'name')
		await store.mutate((current) => {
			const legacyBackup = current.archon.backups.find((item) => item.id === backupId)
			const queueBackup = current.archon.backup_queue.backups.find((item) => item.id === backupId)
			if (!legacyBackup || !queueBackup)
				throw new HttpError(404, `Backup ${backupId} was not found`)
			legacyBackup.name = name
			queueBackup.name = name
			appendAction(current, 'backup_renamed')
		})
		empty(response)
		return true
	}
	if (method === 'POST' && match[2] === '/restore') {
		await store.mutate((current) => appendAction(current, 'backup_restored'))
		empty(response)
		return true
	}
	if (method === 'POST' && match[2] === '/retry') {
		empty(response)
		return true
	}
	return false
}

async function deleteBackups(store: StateStore, ids: string[]): Promise<void> {
	await store.mutate((state) => {
		state.archon.backups = state.archon.backups.filter((item) => !ids.includes(item.id))
		state.archon.backup_queue.backups = state.archon.backup_queue.backups.filter(
			(item) => !ids.includes(item.id),
		)
		appendAction(state, 'backup_deleted')
	})
}

async function handleContent(context: HostingContext): Promise<boolean> {
	const { request, response, url, store } = context
	const method = request.method ?? 'GET'
	const base = `/v1/servers/${SERVER_ID}/worlds/${WORLD_ID}`
	const addonsPath = `${base}/addons`
	const state = store.read()

	if (method === 'GET' && url.pathname === addonsPath) {
		let content = state.archon.content
		if (content.addons) {
			const fromModpack = url.searchParams.get('from_modpack')
			const disabled = url.searchParams.get('disabled')
			content = {
				...content,
				addons: content.addons.filter(
					(item) =>
						(fromModpack === null || item.from_modpack === (fromModpack === 'true')) &&
						(disabled === null || item.disabled === (disabled === 'true')),
				),
			}
		}
		json(response, 200, content)
		return true
	}

	if (
		method === 'POST' &&
		(url.pathname === addonsPath || url.pathname === `${addonsPath}/install-many`)
	) {
		const body = await readJson(request)
		const requests = Array.isArray(body) ? body : [body]
		await store.mutate((current) => {
			current.archon.content.addons ??= []
			for (const value of requests) {
				const item = objectBody(value)
				const projectId = stringField(item, 'project_id')
				if (current.archon.content.addons.some((addon) => addon.project_id === projectId)) continue
				current.archon.content.addons.push({
					id: projectId,
					filename: `${projectId}.jar`,
					filesize: 0,
					disabled: false,
					kind: typeof item.kind === 'string' ? (item.kind as Archon.Content.v1.AddonKind) : 'mod',
					from_modpack: false,
					status: 'installed',
					pack_client_retained: false,
					pack_client_depends: false,
					has_update: null,
					name: projectId,
					project_id: projectId,
					version: {
						id:
							typeof item.version_id === 'string'
								? item.version_id
								: `${projectId}-fixture-version`,
						name: 'Fixture version',
					},
					owner: null,
					icon_url: null,
				})
			}
			appendAction(current, 'addon_added')
		})
		empty(response)
		return true
	}

	const toggleMatch = url.pathname.match(
		new RegExp(`^${addonsPath}/(delete|disable|enable)(-many)?$`),
	)
	if (method === 'POST' && toggleMatch) {
		const body = await readJson(request)
		const rawItems = toggleMatch[2] ? objectBody(body).items : [body]
		const items = Array.isArray(rawItems) ? rawItems.map(objectBody) : []
		const filenames = items.map((item) => stringField(item, 'filename'))
		await store.mutate((current) => {
			if (toggleMatch[1] === 'delete') {
				current.archon.content.addons =
					current.archon.content.addons?.filter((item) => !filenames.includes(item.filename)) ?? []
			} else {
				for (const addon of current.archon.content.addons ?? []) {
					if (filenames.includes(addon.filename)) addon.disabled = toggleMatch[1] === 'disable'
				}
			}
			appendAction(current, `addon_${toggleMatch[1]}d`)
		})
		empty(response)
		return true
	}

	if (method === 'POST' && url.pathname === `${base}/content`) {
		const body = objectBody(await readJson(request))
		await store.mutate((current) => {
			if (body.content_variant === 'bare') {
				if (typeof body.loader === 'string') current.archon.content.modloader = body.loader
				if (typeof body.version === 'string')
					current.archon.content.modloader_version = body.version
				if (typeof body.game_version === 'string')
					current.archon.content.game_version = body.game_version
			}
			appendAction(current, 'server_reset')
		})
		empty(response)
		return true
	}
	if (
		method === 'POST' &&
		(url.pathname === `${base}/content/repair` || url.pathname === `${base}/content/unlink-modpack`)
	) {
		await store.mutate((current) => {
			if (url.pathname.endsWith('unlink-modpack')) current.archon.content.modpack = null
			appendAction(
				current,
				url.pathname.endsWith('repair') ? 'server_repaired' : 'modpack_unlinked',
			)
		})
		empty(response)
		return true
	}

	if (url.pathname === `${base}/content/update-game-version`) {
		const gameVersion = url.searchParams.get('game_version')
		if (!gameVersion) throw new HttpError(400, 'game_version is required')
		if (method === 'GET') {
			json(response, 200, {
				addon_changes: [],
				new_game_version: gameVersion,
				new_loader_version: state.archon.content.modloader_version ?? '',
				has_unknown_content: false,
			} satisfies Archon.Content.v1.UpdateGameVersionPreview)
			return true
		}
		if (method === 'POST') {
			await store.mutate((current) => {
				current.archon.content.game_version = gameVersion
				const world = current.archon.server_v1.worlds.find((item) => item.id === WORLD_ID)
				if (world?.content) world.content.game_version = gameVersion
				current.archon.server_v0.mc_version = gameVersion
				appendAction(current, 'game_version_edited')
			})
			empty(response)
			return true
		}
	}
	return false
}

function appendAction(state: ReturnType<StateStore['read']>, action: string): void {
	state.archon.actions.push({
		actor: { type: 'user', user_id: PERSONAS.owner.user.id },
		action: { action },
		server_id: SERVER_ID,
		world_id: WORLD_ID,
		timestamp: new Date().toISOString(),
	})
}

function requirePersona(persona: Persona | null): asserts persona is Persona {
	if (!persona) throw new HttpError(401, 'Use a proxy-lab fixture token for private service routes')
}

function openSyncStream(response: ServerResponse, state: ReturnType<StateStore['read']>): void {
	response.writeHead(200, {
		'access-control-allow-origin': 'http://localhost:1420',
		'cache-control': 'no-cache',
		connection: 'keep-alive',
		'content-type': 'text/event-stream',
	})
	const events: Archon.Sync.v1.SyncEvent[] = [
		{
			type: 'server.patch',
			name: state.archon.server_v1.name,
			subdomain: state.archon.server_v1.subdomain,
		},
		{
			type: 'server.network.patch',
			ports: state.archon.allocations,
		},
	]
	response.write('retry: 10000\n\n')
	events.forEach((event, index) =>
		response.write(`id: seed-${index + 1}\ndata: ${JSON.stringify(event)}\n\n`),
	)
	const heartbeat = setInterval(() => response.write(': proxy-lab heartbeat\n\n'), 15_000)
	response.once('close', () => clearInterval(heartbeat))
}

const { OWNER_USER_ID, RECIPIENT_USER_ID } = {
	OWNER_USER_ID: PERSONAS.owner.user.id,
	RECIPIENT_USER_ID: PERSONAS.recipient.user.id,
}
