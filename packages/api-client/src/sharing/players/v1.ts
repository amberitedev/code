import { sharing } from '@amberite/contracts'

import { AbstractModule } from '../../core/abstract-module'
import type { SharedInstances } from '../../modules/shared-instances/types'

/**
 * The players of a server: the people it is shared with, who get the content they need to play
 * there. Every call takes the server and the shared instance its world follows. Upstream a
 * server's players are the members of that shared instance, so off a Core these are its `users`
 * and `invites` calls. On a Core they are the server's own players, who read the share without
 * being its members. Both answer in the shared instance's shapes, so the Play tab does not tell
 * them apart.
 */
export class SharedInstancesPlayersV1Module extends AbstractModule {
	public getModuleID(): string {
		return 'sharedinstances_players_v1'
	}

	public async list(
		serverId: string,
		instanceId: string,
	): Promise<SharedInstances.Instances.v1.InstanceUsers> {
		if (!this.client.selfHostedHosting)
			return this.client.sharedinstances.instances_v1.getUsers(instanceId)
		return this.client.backend.call(sharing.ListPlayers, { params: await this.ids(serverId) })
	}

	/** Players whose privacy settings refuse the invite come back in `failed`. */
	public async invite(
		serverId: string,
		instanceId: string,
		userIds: string[],
	): Promise<{ failed: string[] }> {
		if (!this.client.selfHostedHosting)
			return this.client.sharedinstances.instances_v1.inviteUsers(instanceId, userIds)
		return this.client.backend.call(sharing.InvitePlayers, {
			params: await this.ids(serverId),
			body: { ...(await this.server(serverId, instanceId)), user_ids: userIds },
		})
	}

	public async remove(serverId: string, instanceId: string, userIds: string[]): Promise<void> {
		if (!this.client.selfHostedHosting)
			return this.client.sharedinstances.instances_v1.removeUsers(instanceId, userIds)
		return this.client.backend.call(sharing.RemovePlayers, {
			params: await this.ids(serverId),
			body: { user_ids: userIds },
		})
	}

	/** Invite links that make whoever opens them a player. */
	public async links(
		serverId: string,
		instanceId: string,
	): Promise<SharedInstances.Invites.v1.InviteLink[]> {
		if (!this.client.selfHostedHosting)
			return this.client.sharedinstances.invites_v1.list(instanceId)
		return this.client.backend.call(sharing.ListPlayerLinks, { params: await this.ids(serverId) })
	}

	public async createLink(
		serverId: string,
		instanceId: string,
		options: { max_age?: number; max_uses: number },
	): Promise<{ id: string }> {
		if (!this.client.selfHostedHosting)
			return this.client.sharedinstances.invites_v1.create(instanceId, options)
		return this.client.backend.call(sharing.CreatePlayerLink, {
			params: await this.ids(serverId),
			body: { ...(await this.server(serverId, instanceId)), ...options },
		})
	}

	public async deleteLink(serverId: string, instanceId: string, inviteId: string): Promise<void> {
		if (!this.client.selfHostedHosting)
			return this.client.sharedinstances.invites_v1.delete(instanceId, inviteId)
		return this.client.backend.call(sharing.DeletePlayerLink, {
			params: { ...(await this.ids(serverId)), invite_id: inviteId },
		})
	}

	private async ids(serverId: string) {
		const core = await this.client.archon.cores_v1.coreOf(serverId)
		return { core_id: core.id, server_id: serverId }
	}

	/**
	 * What a write tells the backend about the server: the share it follows, the name its players
	 * see, its game port, and where the Core's router opened it on the internet (null while it has
	 * not). Players join that when known, otherwise the port on the Core's latest address.
	 */
	private async server(serverId: string, instanceId: string) {
		const { name, net } = await this.client.archon.servers_v0.get(serverId)
		return {
			instance_id: instanceId,
			name,
			port: net.game_port ?? net.port,
			outside: net.outside ?? null,
		}
	}
}
