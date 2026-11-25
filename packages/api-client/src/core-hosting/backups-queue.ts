import { type BackupsQueue, core } from '@amberite/contracts'

import { ApiCallError } from '../contracts/errors'
import type { AbstractModrinthClient } from '../core/abstract-client'
import type { ArchonBackupsQueueV1Module } from '../modules/archon/backups-queue/v1'
import type { Archon } from '../modules/archon/types'
import { type Adapter, coreOf } from './adapter'

/** A Core's backups with the queue's fields a Core has no use for. */
export function queue(
	listed: BackupsQueue,
): Archon.BackupsQueue.v1.BackupsQueueWithOptionsResponse {
	const legacy = { has_parent: false, synthetic_legacy: false }
	return {
		...listed,
		active_operations: listed.active_operations.map((op) => ({ ...op, ...legacy })),
		backups: listed.backups.map((backup) => ({
			...backup,
			locked: false,
			history: backup.history.map((op) => ({ ...op, ...legacy })),
		})),
	}
}

/** A Core server has one world, whose id is the server's. */
export function backupsQueue(client: AbstractModrinthClient): Adapter<ArchonBackupsQueueV1Module> {
	const list = async (serverId: string) =>
		queue(
			await (await coreOf(client, serverId)).call(core.ListBackups, { params: { id: serverId } }),
		)
	const backup = async (serverId: string, backupId: string) => ({
		routes: await coreOf(client, serverId),
		params: { params: { id: serverId, backup_id: backupId } },
	})
	const operation = async (
		serverId: string,
		operationId: number,
		endpoint: typeof core.AckOperation | typeof core.CancelOperation,
	) => {
		await (
			await coreOf(client, serverId)
		).call(endpoint, {
			params: { id: serverId, operation_id: operationId },
		})
	}
	return {
		list,
		listWithOptions: list,
		async patchOptions(serverId, _worldId, request, acknowledgeRotation = false) {
			const routes = await coreOf(client, serverId)
			// Deleting the excess takes a fresh ticket, as deleting backups by hand does.
			return routes.call(
				{ ...core.ChangeBackupOptions, dangerous: acknowledgeRotation },
				{
					params: { id: serverId },
					query: { ar: acknowledgeRotation || undefined },
					body: request,
				},
			)
		},
		async create(serverId, _worldId, request) {
			return (await coreOf(client, serverId)).call(core.CreateBackup, {
				params: { id: serverId },
				body: { name: request.name },
			})
		},
		ackCreate: (serverId, _worldId, operationId) =>
			operation(serverId, operationId, core.AckOperation),
		cancelCreate: (serverId, _worldId, operationId) =>
			operation(serverId, operationId, core.CancelOperation),
		ackRestore: (serverId, _worldId, operationId) =>
			operation(serverId, operationId, core.AckOperation),
		cancelRestore: (serverId, _worldId, operationId) =>
			operation(serverId, operationId, core.CancelOperation),
		async delete(serverId, _worldId, backupId) {
			const { routes, params } = await backup(serverId, backupId)
			await routes.call(core.DeleteBackup, params)
		},
		async deleteMany(serverId, _worldId, backupIds) {
			await (
				await coreOf(client, serverId)
			).call(core.DeleteBackups, {
				params: { id: serverId },
				body: { backup_ids: backupIds },
			})
		},
		async restore(serverId, _worldId, backupId) {
			const { routes, params } = await backup(serverId, backupId)
			await routes.call(core.RestoreBackup, params)
		},
		async undelete() {
			throw new ApiCallError(501, 'unsupported', 'A Core deletes a backup at once.')
		},
		async retry(serverId, _worldId, backupId) {
			const { routes, params } = await backup(serverId, backupId)
			await routes.call(core.RetryBackup, params)
		},
	}
}
