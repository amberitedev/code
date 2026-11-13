import {
	core,
	type CoreServer,
	type PowerAction,
	type ReachabilityReport,
} from '@amberite/contracts'

import { ApiCallError } from '../contracts/errors'
import type { AbstractModrinthClient } from '../core/abstract-client'
import { ModrinthApiError } from '../core/errors'
import type { ArchonServersV0Module } from '../modules/archon/servers/v0'
import type { Archon } from '../modules/archon/types'
import { coreNodeUrl } from '../utils/node-url'
import { type Adapter, coreOf } from './adapter'

const LOADERS: Record<string, Archon.Servers.v0.Loader> = {
	vanilla: 'Vanilla',
	fabric: 'Fabric',
	quilt: 'Quilt',
	forge: 'Forge',
	neoforge: 'NeoForge',
	paper: 'Paper',
}

/**
 * The Modrinth scope bits a role reads as (`serverPermissionBits` in `@modrinth/ui`): the owner is
 * `SERVER_ADMIN`; editors read, power, console, files, setup, backups and advanced settings;
 * viewers read. Each survives as a JavaScript number.
 */
export const PERMISSIONS = {
	owner: 2 ** 64 - 2 ** 15,
	editor: 2 ** 64 - 2 ** 57,
	viewer: 2 ** 63,
}

const POWER = {
	Start: 'start',
	Stop: 'stop',
	Restart: 'restart',
	Kill: 'kill',
} satisfies Record<Parameters<ArchonServersV0Module['power']>[1], PowerAction>

const JDK_MAJORS = { lts8: 8, lts11: 11, lts17: 17, lts21: 21 } as const
type JdkName = keyof typeof JDK_MAJORS

/** The nearest name for a Java major. Java 25 has none and reads as `lts21`. */
function jdkName(major: number): JdkName {
	if (major <= 8) return 'lts8'
	if (major <= 11) return 'lts11'
	if (major <= 17) return 'lts17'
	return 'lts21'
}

function isJdkName(name: string): name is JdkName {
	return name in JDK_MAJORS
}

/** Shown on the server's page until the owner accepts the EULA; no server starts before that. */
const EULA_NOTICE: Archon.Servers.v0.Notice = {
	id: 0,
	dismissable: false,
	title: 'Minecraft EULA',
	message:
		'Servers on this Core start once its owner accepts the Minecraft EULA: run `copal eula` on the Core to read it, then `copal eula --accept`.',
	level: 'warn',
	announced: '',
}

function reachability(report: ReachabilityReport): Archon.Servers.v0.Reachability {
	return { state: report.state, cause: report.state === 'reachable' ? null : report.title }
}

/** `server` in the hosting API's shape. Its node is the Core, at `base`. See `coreNodeUrl`. */
export function archonServer(server: CoreServer, base: string): Archon.Servers.v0.Server {
	return {
		server_id: server.id,
		name: server.name,
		owner_id: server.owner_id ?? '',
		net: { ...server.net, domain: '' },
		game: 'Minecraft',
		backup_quota: 0,
		used_backup_quota: 0,
		status: server.installing || !server.game_version ? 'installing' : 'available',
		suspension_reason: null,
		loader: (server.loader && LOADERS[server.loader]) || null,
		loader_version: server.loader_version,
		mc_version: server.game_version,
		upstream: null,
		sftp_username: '',
		sftp_password: '',
		sftp_host: '',
		datacenter: '',
		notices: server.eula_accepted ? [] : [EULA_NOTICE],
		node: { token: '', instance: coreNodeUrl(base, server.id) },
		flows: { intro: false },
		is_medal: false,
		current_user_permissions: PERMISSIONS[server.role ?? 'owner'],
		locked_since: null,
		icon: null,
		linked_instance: server.linked_instance && {
			...server.linked_instance,
			visible_to_friends: false,
		},
		power_state: server.power,
		reachability: server.reachability && reachability(server.reachability),
		players: server.players,
	}
}

/** The server's setup, or `conflict` while it is not installed. */
async function installedSetup(client: AbstractModrinthClient, serverId: string) {
	const setup = await (
		await coreOf(client, serverId)
	).call(core.GetSetup, {
		params: { id: serverId },
	})
	if (setup.invocation === null || setup.java_version === null)
		throw new ApiCallError(409, 'conflict', 'The server is not installed yet.')
	return { invocation: setup.invocation, major: setup.java_version }
}

/** Core builds the startup command itself; memory and flags are set per server. */
export function sameCommand(command: string | null | undefined, current: string): void {
	if (command && command.trim() !== current)
		throw new ApiCallError(
			400,
			'invalid_input',
			'Core builds the startup command itself; memory and flags are set per server.',
		)
}

export function serversV0(client: AbstractModrinthClient): Adapter<ArchonServersV0Module> {
	const cores = client.archon.cores_v1
	const base = (serverId: string) => cores.hostingUrl(`/servers/${serverId}`)
	return {
		async get(serverId) {
			const routes = await coreOf(client, serverId)
			const server = await routes.call(core.GetServer, { params: { id: serverId } })
			return archonServer(server, await base(serverId))
		},
		list: () => cores.servers(),
		async updateName(serverId, name) {
			await (
				await coreOf(client, serverId)
			).call(core.ChangeServer, {
				params: { id: serverId },
				body: { name, update_channel: null },
			})
		},
		async getFilesystemAuth(serverId) {
			const auth = await client.request<Archon.Servers.v0.JWTAuth>(`/servers/${serverId}/fs`, {
				api: 'archon',
				version: 'modrinth/v0',
				method: 'GET',
			})
			return { ...auth, url: coreNodeUrl(await base(serverId), serverId) }
		},
		async getWebSocketAuth(serverId) {
			// A Core's `/events` socket, at the base requests to it go to. See `CoreWebSocketClient`.
			return { url: `${(await base(serverId)).replace(/^http/, 'ws')}/events`, token: '' }
		},
		async power(serverId, action) {
			const change = { params: { id: serverId }, body: { action: POWER[action] } }
			await (await coreOf(client, serverId)).call(core.ChangePower, change).catch(eulaFirst)
		},
		async reinstall(serverId, request, hardReset = false) {
			if (hardReset || !('loader' in request) || !request.game_version)
				throw new ApiCallError(
					400,
					'invalid_input',
					'Core installs a loader on a server and never erases it. Install packs through its content.',
				)
			await (
				await coreOf(client, serverId)
			).call(core.InstallLoader, {
				params: { id: serverId },
				body: {
					loader: request.loader.toLowerCase(),
					loader_version: request.loader_version ?? null,
					game_version: request.game_version,
				},
			})
		},
		async getStartupConfig(serverId) {
			const { invocation, major } = await installedSetup(client, serverId)
			return {
				invocation,
				original_invocation: invocation,
				jdk_version: jdkName(major),
				// Core runs Azul Zulu, which this union has no name for.
				jdk_build: 'temurin',
			}
		},
		async updateStartupConfig(serverId, config) {
			const { invocation, major } = await installedSetup(client, serverId)
			sameCommand(config.invocation, invocation)
			// The panel sends back what it read, and that name is lossy: the same name is no change.
			if (config.jdk_version === jdkName(major)) return
			const chosen = config.jdk_version && isJdkName(config.jdk_version) ? config.jdk_version : null
			await (
				await coreOf(client, serverId)
			).call(core.SetJavaVersion, {
				params: { id: serverId },
				body: { major: chosen && JDK_MAJORS[chosen] },
			})
		},
	}
}

/** The panel shows a power failure's message; a missing EULA is the owner's to accept. */
function eulaFirst(error: unknown): never {
	if (error instanceof ApiCallError && error.code === 'eula_not_accepted')
		throw new ModrinthApiError(error.description, {
			statusCode: 409,
			responseData: { error: error.code, description: error.description },
		})
	throw error
}
