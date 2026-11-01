import { AbstractModule } from '../../../core/abstract-module'
import type { RequestOptions } from '../../../types/request'
import { SharedInstancesApiError } from '../errors'
import type { SharedInstances } from '../types'

/**
 * The players of a server: the people it is shared with, who get the content they need to play
 * there. Every call takes the server and the shared instance its world follows. Upstream a
 * server's players are the members of that shared instance, so off a Core these are its `users`
 * and `invites` calls. On a Core they are the server's own players, who read the share without
 * being its members (docs/internals/plan.md, "Shares, members, and players"). Both answer in the
 * shared instance's shapes, so the Play tab does not tell them apart.
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
		return this.request(await this.path(serverId, 'players'), { method: 'GET' })
	}

	/** Players whose privacy settings refuse the invite come back in `failed`. */
	public async invite(
		serverId: string,
		instanceId: string,
		userIds: string[],
	): Promise<{ failed: string[] }> {
		if (!this.client.selfHostedHosting)
			return this.client.sharedinstances.instances_v1.inviteUsers(instanceId, userIds)
		return this.request(await this.path(serverId, 'players'), {
			method: 'POST',
			body: { ...(await this.server(serverId, instanceId)), user_ids: userIds },
			retry: false,
		})
	}

	public async remove(serverId: string, instanceId: string, userIds: string[]): Promise<void> {
		if (!this.client.selfHostedHosting)
			return this.client.sharedinstances.instances_v1.removeUsers(instanceId, userIds)
		return this.request(await this.path(serverId, 'players'), {
			method: 'DELETE',
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
		return this.request(await this.path(serverId, 'invites'), { method: 'GET' })
	}

	public async createLink(
		serverId: string,
		instanceId: string,
		options: { max_age?: number; max_uses: number },
	): Promise<{ id: string }> {
		if (!this.client.selfHostedHosting)
			return this.client.sharedinstances.invites_v1.create(instanceId, options)
		return this.request(await this.path(serverId, 'invites'), {
			method: 'POST',
			body: { ...(await this.server(serverId, instanceId)), ...options },
			retry: false,
		})
	}

	public async deleteLink(serverId: string, instanceId: string, inviteId: string): Promise<void> {
		if (!this.client.selfHostedHosting)
			return this.client.sharedinstances.invites_v1.delete(instanceId, inviteId)
		return this.request(`${await this.path(serverId, 'invites')}/${encodeURIComponent(inviteId)}`, {
			method: 'DELETE',
		})
	}

	private async request<T>(
		path: string,
		options: Omit<RequestOptions, 'api' | 'version'>,
	): Promise<T> {
		try {
			return await this.client.request<T>(path, { ...options, api: 'sharedinstances', version: 1 })
		} catch (error) {
			throw SharedInstancesApiError.from(error)
		}
	}

	private async path(serverId: string, section: 'players' | 'invites') {
		const core = await this.client.archon.cores_v1.coreOf(serverId)
		return `/servers/${encodeURIComponent(core.id)}/${encodeURIComponent(serverId)}/${section}`
	}

	/**
	 * What a write tells the backend about the server: the share it follows, the name its players
	 * see, and its port. The backend joins the port to the Core's latest address for players.
	 */
	private async server(serverId: string, instanceId: string) {
		const { name, net } = await this.client.archon.servers_v0.get(serverId)
		return { instance_id: instanceId, name, port: net.port }
	}
}
