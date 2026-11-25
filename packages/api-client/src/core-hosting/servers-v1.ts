import { core, instanceSharing, type Performance, type Restarts } from '@amberite/contracts'

import type { AbstractModrinthClient } from '../core/abstract-client'
import type { ArchonServersV1Module } from '../modules/archon/servers/v1'
import type { Archon } from '../modules/archon/types'
import { coreNodeUrl } from '../utils/node-url'
import { type Adapter, coreOf } from './adapter'

declare module '../modules/archon/servers/v1' {
	interface ArchonServersV1Module {
		/** Core only. Deletes a stopped server, with its world, files and backups. */
		deleteLocal(serverId: string): Promise<void>
		/** Core only. A server's memory, JVM flags and restart settings. */
		getPerformance(serverId: string): Promise<Performance>
		/** Core only. When a server restarts on its own. */
		setRestarts(serverId: string, restarts: Restarts): Promise<Performance>
	}
}

/** A Core server has one world, whose id is the server's. */
export function serversV1(client: AbstractModrinthClient): Adapter<ArchonServersV1Module> {
	async function get(serverId: string): Promise<Archon.Servers.v1.ServerFull> {
		const routes = await coreOf(client, serverId)
		const params = { params: { id: serverId } }
		const [server, startup, machine, base] = await Promise.all([
			routes.call(core.GetServer, params),
			routes.call(core.GetStartup, params),
			routes.call(core.GetMachine),
			client.cores.serverUrl(serverId),
		])
		// Best effort: a push would change something.
		const preview =
			server.shared_instance_id === null
				? null
				: await routes
						.call(instanceSharing.GetSharedInstancePublishPreview, params)
						.catch(() => null)
		const host = new URL(base).host
		return {
			id: server.server_id,
			name: server.name,
			subdomain: '',
			specs: { cpu: machine.cpus, memory_mb: server.memory_mb, storage_mb: 0, swap_mb: 0 },
			sftp_username: '',
			sftp_password: '',
			tags: [],
			drop_client_mods: false,
			location: {
				status: 'assigned',
				location_metadata: {
					region: '',
					region_should_be_user_displayed: false,
					hostname: host,
					url_host: coreNodeUrl(base, server.server_id),
					is_decommissioned_node: false,
				},
			},
			icon: null,
			worlds: [
				{
					id: server.server_id,
					name: server.name,
					update_channel: server.update_channel,
					created_at: server.created_at,
					is_active: true,
					download_method: { method_type: 'direct_node_download' },
					backups: [],
					content:
						server.loader && server.mc_version
							? {
									shared_instance_id: server.shared_instance_id,
									shared_instance_needs_update: (preview?.diffs.length ?? 0) > 0,
									modloader: server.loader.toLowerCase(),
									modloader_version: server.loader_version ?? '',
									game_version: server.mc_version,
									java_version: startup.java_version,
									invocation: startup.original_invocation,
									original_invocation: startup.original_invocation,
								}
							: null,
					readiness: { data_synchronized_fetched: true },
				},
			],
		}
	}

	return {
		get,
		async list() {
			const { servers } = await client.cores.servers()
			return Promise.all(servers.map((server) => get(server.server_id)))
		},
		async patchWorld(serverId, _worldId, body) {
			if (!body.update_channel) return
			await (
				await coreOf(client, serverId)
			).call(core.ChangeServer, {
				params: { id: serverId },
				body: { name: null, update_channel: body.update_channel },
			})
		},
		// The world downloads straight from the Core.
		selectWorldDownload: async () => ({ method_type: 'direct_node_download' }),
		async deleteLocal(serverId) {
			await (await coreOf(client, serverId)).call(core.DeleteServer, { params: { id: serverId } })
		},
		// A Core server never shows the intro (`flows.intro` is false), so there is none to end.
		async endIntro() {},
		async getPerformance(serverId) {
			return (await coreOf(client, serverId)).call(core.GetPerformance, {
				params: { id: serverId },
			})
		},
		async setRestarts(serverId, restarts) {
			return (await coreOf(client, serverId)).call(core.SetRestarts, {
				params: { id: serverId },
				body: restarts,
			})
		},
	}
}
