import {
	type BackupOperation,
	core,
	type CoreUser,
	type OperationState,
	type ServerBackup,
	type ServerBackups,
} from '@amberite/contracts'

import { ApiCallError } from '../contracts/errors'
import type { AbstractModrinthClient } from '../core/abstract-client'
import type { ArchonBackupsQueueV1Module } from '../modules/archon/backups-queue/v1'
import type { Archon } from '../modules/archon/types'
import { type Adapter, coreOf } from './adapter'

type Queue = Archon.BackupsQueue.v1.BackupsQueueResponse

const STATUS = {
	queued: 'pending',
	running: 'in_progress',
	done: 'done',
	failed: 'error',
} satisfies Record<ServerBackup['state'], Archon.BackupsQueue.v1.BackupStatus>

const OPERATION_STATE = {
	queued: 'pending',
	running: 'ongoing',
	done: 'completed',
	failed: 'failed',
	cancelled: 'cancelled',
} satisfies Record<OperationState, Archon.BackupsQueue.v1.BackupQueueState>

function time(seconds: number): string {
	return new Date(seconds * 1000).toISOString()
}

function userInfo(user: CoreUser | null): Archon.BackupsQueue.v1.UserInfo | null {
	return user && { id: user.id, username: user.name, avatar_url: user.avatar_url }
}

function active(op: BackupOperation): boolean {
	return op.state === 'queued' || op.state === 'running'
}

/** A Core's backups as the queue shows them. */
export function queue({ backups }: ServerBackups): Queue {
	return {
		active_operations: backups.flatMap((backup) =>
			backup.operations.filter(active).map((op) => ({
				backup_id: backup.id,
				operation_type: op.kind,
				operation_id: op.id,
				has_parent: false,
				scheduled_for: time(op.queued_at),
				started_at: op.started_at === null ? null : time(op.started_at),
				synthetic_legacy: false,
				user_info: userInfo(op.user),
			})),
		),
		backups: backups.map((backup) => ({
			id: backup.id,
			name: backup.name,
			created_at: time(backup.created_at),
			status: STATUS[backup.state],
			locked: false,
			automated: backup.kind !== 'manual',
			...(backup.rotates_at === null ? {} : { rotates_at: time(backup.rotates_at) }),
			history: backup.operations.map((op) => ({
				operation_type: op.kind,
				operation_id: op.id,
				state: OPERATION_STATE[op.state],
				scheduled_for: time(op.queued_at),
				started_at: op.started_at === null ? null : time(op.started_at),
				completed_at: op.ended_at === null ? null : time(op.ended_at),
				has_parent: false,
				error: op.error,
				should_prompt: !active(op) && !op.acked,
				synthetic_legacy: false,
				user_info: userInfo(op.user),
			})),
		})),
	}
}

/** A Core server has one world, whose id is the server's. */
export function backupsQueue(client: AbstractModrinthClient): Adapter<ArchonBackupsQueueV1Module> {
	const list = async (serverId: string) =>
		(await coreOf(client, serverId)).call(core.ListBackups, { params: { id: serverId } })
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
		list: async (serverId) => queue(await list(serverId)),
		async listWithOptions(serverId) {
			const backups = await list(serverId)
			const { schedule } = backups
			const listed = queue(backups)
			const live = listed.backups.filter((each) => each.status !== 'error')
			return {
				...listed,
				options: {
					split_automatic: schedule.keep,
					split_manual: schedule.slots - schedule.keep,
					limit: schedule.slots,
					interval: schedule.interval_hours,
				},
				next_auto_backup_at:
					backups.next_scheduled_at === null ? null : time(backups.next_scheduled_at),
				manual_count: live.filter((each) => !each.automated).length,
				auto_count: listed.backups.filter((each) => each.rotates_at !== undefined).length,
			}
		},
		async patchOptions(serverId, _worldId, request, acknowledgeRotation = false) {
			const { schedule } = await list(serverId)
			const { split_automatic: automatic, split_manual: manual } = request
			if (automatic !== undefined && manual !== undefined && automatic + manual !== schedule.slots)
				throw new ApiCallError(
					400,
					'invalid_input',
					`Scheduled and manual slots add up to ${schedule.slots}.`,
				)
			const keep = automatic ?? (manual === undefined ? null : schedule.slots - manual)
			const routes = await coreOf(client, serverId)
			// Deleting the excess takes a fresh ticket, as deleting backups by hand does.
			const change = await routes.call(
				{ ...core.ChangeBackupSchedule, dangerous: acknowledgeRotation },
				{
					params: { id: serverId },
					body: {
						interval_hours: request.interval ?? null,
						keep,
						delete_excess: acknowledgeRotation,
					},
				},
			)
			return {
				applied: change.applied,
				...(change.excess.length ? { backup_ids: change.excess } : {}),
			}
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
				body: { ids: backupIds },
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
