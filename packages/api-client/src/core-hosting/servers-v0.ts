import { core, type Loader, type Server } from '@amberite/contracts'

import { ApiCallError } from '../contracts/errors'
import type { AbstractModrinthClient } from '../core/abstract-client'
import type { ArchonServersV0Module } from '../modules/archon/servers/v0'
import type { Archon } from '../modules/archon/types'
import { coreNodeUrl } from '../utils/node-url'
import { type Adapter, coreOf, opOn } from './adapter'

const LOADERS: Loader[] = ['Vanilla', 'Fabric', 'Quilt', 'Forge', 'NeoForge', 'Paper']

/** The Core's loader `name` names, in any case. */
export function loader(name: string): Loader {
	const found = LOADERS.find((each) => each.toLowerCase() === name.replace('_', '').toLowerCase())
	if (!found)
		throw new ApiCallError(
			400,
			'invalid_input',
			'Core installs Vanilla, Fabric, Quilt, Forge, NeoForge, and Paper.',
		)
	return found
}

/**
 * The Modrinth scope bits of `SERVER_ADMIN` (`serverPermissionBits` in `@modrinth/ui`), which
 * survive as a JavaScript number. Core answers only its owner.
 */
const OWNER_PERMISSIONS = 2 ** 64 - 2 ** 15

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

/**
 * `server` with the hosting API's fields a Core has no use for. Its node is the Core, at `base`.
 * See `coreNodeUrl`. The panel shows a server that is not set up as installing.
 */
export function archonServer(server: Server, base: string): Archon.Servers.v0.Server {
	return {
		...server,
		owner_id: server.owner_id ?? '',
		net: { ...server.net, domain: '' },
		game: 'Minecraft',
		backup_quota: 0,
		used_backup_quota: 0,
		status: server.flows.intro ? 'installing' : server.status,
		flows: { intro: false },
		suspension_reason: null,
		sftp_username: '',
		sftp_password: '',
		sftp_host: '',
		datacenter: '',
		notices: [],
		node: { token: '', instance: coreNodeUrl(base, server.server_id) },
		is_medal: false,
		current_user_permissions: OWNER_PERMISSIONS,
		locked_since: null,
		reachability: server.reachability && {
			state: server.reachability.state,
			cause: server.reachability.state === 'reachable' ? null : server.reachability.title,
		},
	}
}

/** The server's startup, or `conflict` while it is not installed. */
async function installedStartup(client: AbstractModrinthClient, serverId: string) {
	const startup = await (
		await coreOf(client, serverId)
	).call(core.GetStartup, {
		params: { id: serverId },
	})
	if (startup.original_invocation === null || startup.java_version === null)
		throw new ApiCallError(409, 'conflict', 'The server is not installed yet.')
	return { invocation: startup.original_invocation, major: startup.java_version }
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
	const cores = client.cores
	const base = (serverId: string) => cores.serverUrl(serverId)
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
		// The file routes check the session themselves, so there is no token.
		getFilesystemAuth: async (serverId) => ({
			url: coreNodeUrl(await base(serverId), serverId),
			token: '',
		}),
		async getWebSocketAuth(serverId) {
			// A Core's `/events` socket, at the base requests to it go to. See `CoreWebSocketClient`.
			return { url: `${(await base(serverId)).replace(/^http/, 'ws')}/events`, token: '' }
		},
		async power(serverId, action) {
			await opOn(client, serverId, (routes, op) =>
				routes.call(core.ChangePower, { params: { id: serverId }, body: { action, op } }),
			)
		},
		async reinstall(serverId, request, hardReset = false) {
			if (hardReset || !('loader' in request) || !request.game_version)
				throw new ApiCallError(
					400,
					'invalid_input',
					'Core installs a loader on a server and never erases it. Install packs through its content.',
				)
			const game_version = request.game_version
			await opOn(client, serverId, (routes, op) =>
				routes.call(core.InstallLoader, {
					params: { id: serverId },
					body: {
						loader: loader(request.loader),
						loader_version: request.loader_version ?? null,
						game_version,
						op,
					},
				}),
			)
		},
		async getStartupConfig(serverId) {
			const { invocation, major } = await installedStartup(client, serverId)
			return {
				invocation,
				original_invocation: invocation,
				jdk_version: jdkName(major),
				// Core runs Azul Zulu, which this union has no name for.
				jdk_build: 'temurin',
			}
		},
		async updateStartupConfig(serverId, config) {
			const { invocation, major } = await installedStartup(client, serverId)
			sameCommand(config.invocation, invocation)
			// The panel sends back what it read, and that name is lossy: the same name is no change.
			if (config.jdk_version === jdkName(major)) return
			const chosen = config.jdk_version && isJdkName(config.jdk_version) ? config.jdk_version : null
			await (
				await coreOf(client, serverId)
			).call(core.ChangeStartup, {
				params: { id: serverId },
				body: { java_version: chosen && JDK_MAJORS[chosen] },
			})
		},
	}
}
