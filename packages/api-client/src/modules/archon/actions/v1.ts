import { AbstractModule } from '../../../core/abstract-module'
import { actionLog, onlyForOwner } from '../../copal/access/archon'
import type { Archon } from '../types'

export class ArchonActionsV1Module extends AbstractModule {
	public getModuleID(): string {
		return 'archon_actions_v1'
	}

	/**
	 * Get server action log entries.
	 * GET /v1/servers/:server_id/action-log
	 */
	public async list(
		serverId: string,
		options: Archon.Actions.v1.ListActionLogOptions = {},
	): Promise<Archon.Actions.v1.ActionLogResponse> {
		if (this.client.selfHostedHosting) {
			// A Core's activity log, the owner's only. Its members name the users it mentions.
			const access = this.client.copal.access_v1
			const coreId = (await this.client.archon.cores_v1.coreOf(serverId)).id
			const [page, members] = await Promise.all([
				access
					.activity(coreId, {
						server_id: serverId,
						limit: options.limit,
						before: options.offset || undefined,
					})
					.catch(onlyForOwner),
				access.serverAccess(coreId, serverId).catch(onlyForOwner),
			])
			return actionLog(page ?? { entries: [], next_before: null }, members, options)
		}
		const params: Record<string, string | number> = {}
		if (options.filter) params.filter = JSON.stringify(options.filter)
		if (options.limit !== undefined) params.limit = options.limit
		if (options.offset !== undefined) params.offset = options.offset
		if (options.order !== undefined) params.order = options.order
		if (options.min_datetime !== undefined) params.min_datetime = options.min_datetime
		if (options.max_datetime !== undefined) params.max_datetime = options.max_datetime

		return this.client.request<Archon.Actions.v1.ActionLogResponse>(
			`/servers/${serverId}/action-log`,
			{
				api: 'archon',
				version: 1,
				method: 'GET',
				params: Object.keys(params).length > 0 ? params : undefined,
			},
		)
	}

	/** DELETE /v1/servers/:server_id/action-log. Modrinth Hosting only: a Core keeps its log. */
	public async clear(serverId: string): Promise<void> {
		return this.client.request<void>(`/servers/${serverId}/action-log`, {
			api: 'archon',
			version: 1,
			method: 'DELETE',
		})
	}
}
