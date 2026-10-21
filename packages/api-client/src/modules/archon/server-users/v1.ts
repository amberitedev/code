import { AbstractModule } from '../../../core/abstract-module'
import { ModrinthApiError } from '../../../core/errors'
import { copalRole, onlyForOwner, serverUser, serverUsers } from '../../copal/access/archon'
import type { Archon } from '../types'

/**
 * Who has access to a server. With self-hosted hosting a server is on a Core, which keeps its own
 * access list: these calls go to `copal.access_v1` there, with a fresh ticket where Core asks for
 * one, and answer in the same shapes.
 */
export class ArchonServerUsersV1Module extends AbstractModule {
	public getModuleID(): string {
		return 'archon_server_users_v1'
	}

	/**
	 * Get list of users with access to a server
	 * GET /v1/servers/:server_id/users
	 */
	public async list(serverId: string): Promise<Archon.ServerUsers.v1.ServerUser[]> {
		if (this.client.selfHostedHosting) {
			const access = this.client.copal.access_v1
			const coreId = await this.coreId(serverId)
			// Core lists a server's members to its owner only. A member sees themselves.
			const listed = await access.serverAccess(coreId, serverId).catch(onlyForOwner)
			if (listed) return serverUsers(listed)
			const me = await access.me(coreId)
			const role = me.servers.find((server) => server.server_id === serverId)?.role
			return role ? [serverUser(me.user, role, null)] : []
		}
		return this.client.request<Archon.ServerUsers.v1.ServerUser[]>(`/servers/${serverId}/users`, {
			api: 'archon',
			version: 1,
			method: 'GET',
		})
	}

	/**
	 * Add a user to a server
	 * POST /v1/servers/:server_id/users
	 */
	public async add(
		serverId: string,
		user: Archon.ServerUsers.v1.AddServerUserRequest,
	): Promise<void> {
		if (this.client.selfHostedHosting) {
			// Core shows the name until the invited user signs in and their own ticket names them.
			const invited = await this.client.labrinth.users_v3.get(user.user_id)
			await this.client.copal.access_v1.invite(await this.coreId(serverId), serverId, {
				user: { id: invited.id, name: invited.username, avatar_url: invited.avatar_url ?? null },
				role: copalRole(user.role),
			})
			return
		}
		await this.client.request(`/servers/${serverId}/users`, {
			api: 'archon',
			version: 1,
			method: 'POST',
			body: user,
		})
	}

	/**
	 * Re-send an invite to a pending server user.
	 * POST /v1/servers/:server_id/users/:user_id/reinvite
	 */
	public async reinvite(
		serverId: string,
		userId: string,
	): Promise<Archon.ServerUsers.v1.ReinviteResponse> {
		// A Core grants access with the invite, so nobody on a Core is ever pending.
		if (this.client.selfHostedHosting)
			throw new ModrinthApiError('A Core has no pending invites', { statusCode: 400 })
		return this.client.request<Archon.ServerUsers.v1.ReinviteResponse>(
			`/servers/${serverId}/users/${userId}/reinvite`,
			{
				api: 'archon',
				version: 1,
				method: 'POST',
			},
		)
	}

	/**
	 * Remove a user from a server
	 * DELETE /v1/servers/:server_id/users/:user_id
	 */
	public async delete(serverId: string, userId: string): Promise<void> {
		if (this.client.selfHostedHosting)
			return this.client.copal.access_v1.remove(await this.coreId(serverId), serverId, userId)
		await this.client.request(`/servers/${serverId}/users/${userId}`, {
			api: 'archon',
			version: 1,
			method: 'DELETE',
		})
	}

	/** Core only. The signed-in account gives up its own access to a server it was invited to. */
	public async leave(serverId: string): Promise<void> {
		if (!this.client.selfHostedHosting)
			throw new ModrinthApiError('Only a server on a Core can be left', { statusCode: 400 })
		await this.client.copal.access_v1.leave(await this.coreId(serverId), serverId)
	}

	/**
	 * Update a user's server role
	 * PATCH /v1/servers/:server_id/users/:user_id
	 */
	public async update(
		serverId: string,
		userId: string,
		role: Archon.ServerUsers.v1.AssignableServerUserRole,
	): Promise<void> {
		if (this.client.selfHostedHosting) {
			const coreId = await this.coreId(serverId)
			await this.client.copal.access_v1.changeRole(coreId, serverId, userId, copalRole(role))
			return
		}
		await this.client.request(`/servers/${serverId}/users/${userId}`, {
			api: 'archon',
			version: 1,
			method: 'PATCH',
			body: JSON.stringify(role),
		})
	}

	private async coreId(serverId: string): Promise<string> {
		return (await this.client.archon.cores_v1.coreOf(serverId)).id
	}
}
