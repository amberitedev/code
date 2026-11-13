import { core, type Performance, type Restarts } from '@amberite/contracts'

import { ApiCallError } from '../contracts/errors'
import type { AbstractModrinthClient } from '../core/abstract-client'
import type { ArchonServersV1Module } from '../modules/archon/servers/v1'
import type { Archon } from '../modules/archon/types'
import { coreNodeUrl } from '../utils/node-url'
import { type Adapter, coreOf } from './adapter'

declare module '../modules/archon/servers/v1' {
	interface ArchonServersV1Module {
		/** Core only. Deletes a stopped server, with its world, files and backups. */
		deleteLocal(serverId: string): Promise<void>
		/** Core only. Answers `unsupported`: a Core decides who sees a linked server. */
		setLinkedInstance(
			serverId: string,
			settings: { visible_to_friends?: boolean; shared_instance?: { id: string | null } },
		): Promise<void>
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
		const [server, setup, base] = await Promise.all([
			routes.call(core.GetServer, params),
			routes.call(core.GetSetup, params),
			client.archon.cores_v1.hostingUrl(`/servers/${serverId}`),
		])
		const host = new URL(base).host
		return {
			id: server.id,
			name: server.name,
			subdomain: '',
			specs: { cpu: setup.cpus, memory_mb: server.memory_mb, storage_mb: 0, swap_mb: 0 },
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
					url_host: coreNodeUrl(base, server.id),
					is_decommissioned_node: false,
				},
			},
			icon: null,
			worlds: [
				{
					id: server.id,
					name: server.name,
					update_channel: server.update_channel,
					created_at: new Date(server.created_at * 1000).toISOString(),
					is_active: true,
					download_method: { method_type: 'direct_node_download' },
					backups: [],
					content:
						server.loader && server.game_version
							? {
									shared_instance_id: setup.shared_instance_id,
									shared_instance_needs_update: setup.shared_instance_needs_update,
									modloader: server.loader,
									modloader_version: server.loader_version ?? '',
									game_version: server.game_version,
									java_version: setup.java_version,
									invocation: setup.invocation,
									original_invocation: setup.invocation,
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
			const { servers } = await client.archon.cores_v1.servers()
			return Promise.all(servers.map((server) => get(server.server_id)))
		},
		async patchWorld(serverId, _worldId, body) {
			if (!body.update_channel) return
			await (
				await coreOf(client, serverId)
			).call(core.ChangeServer, {
				params: { id: serverId },
				body: { update_channel: body.update_channel },
			})
		},
		// The world downloads straight from the Core.
		selectWorldDownload: async () => ({ method_type: 'direct_node_download' }),
		async deleteLocal(serverId) {
			await (await coreOf(client, serverId)).call(core.DeleteServer, { params: { id: serverId } })
		},
		async setLinkedInstance() {
			throw new ApiCallError(501, 'unsupported', 'A Core decides who sees a linked server.')
		},
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
