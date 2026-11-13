import { core } from '@amberite/contracts'

import type { AbstractModrinthClient } from '../core/abstract-client'
import { ModrinthApiError } from '../core/errors'
import type { ArchonServerUsersV1Module } from '../modules/archon/server-users/v1'
import { copalRole, onlyForOwner, serverUser, serverUsers as users } from './access'
import { type Adapter, coreOf } from './adapter'

declare module '../modules/archon/server-users/v1' {
	interface ArchonServerUsersV1Module {
		/** Core only. The signed-in account gives up its own access to a server it was invited to. */
		leave(serverId: string): Promise<void>
	}
}

/** A Core keeps its own access list, granted at once by an invite, so nobody is ever pending. */
export function serverUsers(client: AbstractModrinthClient): Adapter<ArchonServerUsersV1Module> {
	return {
		async list(serverId) {
			const access = await coreOf(client, serverId)
			// Core lists a server's members to its owner only. A member sees themselves.
			const listed = await access
				.call(core.ListMembers, { params: { id: serverId } })
				.catch(onlyForOwner)
			if (listed) return users(listed)
			const me = await access.call(core.GetMyAccess)
			const role = me.servers.find((server) => server.server_id === serverId)?.role
			return role ? [serverUser(me.user, role, null)] : []
		},
		async add(serverId, user) {
			// Core shows the name until the invited user signs in and their own ticket names them.
			const invited = await client.labrinth.users_v3.get(user.user_id)
			await (
				await coreOf(client, serverId)
			).call(core.InviteMember, {
				params: { id: serverId },
				body: {
					user: { id: invited.id, name: invited.username, avatar_url: invited.avatar_url ?? null },
					role: copalRole(user.role),
				},
			})
		},
		async reinvite() {
			throw new ModrinthApiError('A Core has no pending invites', { statusCode: 400 })
		},
		async delete(serverId, userId) {
			await (
				await coreOf(client, serverId)
			).call(core.RemoveMember, { params: { id: serverId, user_id: userId } })
		},
		async update(serverId, userId, role) {
			await (
				await coreOf(client, serverId)
			).call(core.ChangeRole, {
				params: { id: serverId, user_id: userId },
				body: { role: copalRole(role) },
			})
		},
		async leave(serverId) {
			const { id } = await client.archon.cores_v1.coreOf(serverId)
			await client.archon.cores_v1.leave(id, serverId)
		},
	}
}
