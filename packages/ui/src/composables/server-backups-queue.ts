import type { CoreBackup, CoreBackupOperation } from '@amberite/amberite-api'
import type { Archon } from '@modrinth/api-client'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, type Ref } from 'vue'

import { type BusyReason, injectHostingBackend } from '#ui/providers'

import { defineMessage } from './i18n'

type CoreBackupsQueueResponse = Archon.BackupsQueue.v1.BackupsQueueResponse

function toActiveOperation(
	operation: CoreBackupOperation,
	backup: CoreBackup | undefined,
): Archon.BackupsQueue.v1.ActiveOperation {
	return {
		backup_id: operation.backup_id,
		operation_type: operation.operation_type,
		operation_id: null,
		has_parent: false,
		scheduled_for: backup?.created_at ?? new Date().toISOString(),
		started_at: backup?.created_at ?? null,
		synthetic_legacy: true,
		user_info: null,
	}
}

function toBackupQueueBackup(
	backup: CoreBackup,
	activeOperation: CoreBackupOperation | undefined,
): Archon.BackupsQueue.v1.BackupQueueBackup {
	const activeHistory = activeOperation
		? [
				{
					operation_type: activeOperation.operation_type,
					operation_id: null,
					state: 'ongoing' as const,
					scheduled_for: backup.created_at,
					started_at: backup.created_at,
					completed_at: null,
					has_parent: false,
					error: null,
					should_prompt: false,
					synthetic_legacy: true,
					user_info: null,
				},
			]
		: []

	return {
		id: backup.id,
		name: backup.name,
		created_at: backup.created_at,
		status: backup.status === 'done' ? 'done' : 'in_progress',
		locked: backup.locked,
		automated: backup.automated,
		history: activeHistory,
	}
}

function toBackupsQueueResponse(response: {
	backups: CoreBackup[]
	active_operations: CoreBackupOperation[]
}): CoreBackupsQueueResponse {
	const backupById = new Map(response.backups.map((backup) => [backup.id, backup]))
	const activeByBackupId = new Map(
		response.active_operations.map((operation) => [operation.backup_id, operation]),
	)

	return {
		active_operations: response.active_operations.map((operation) =>
			toActiveOperation(operation, backupById.get(operation.backup_id)),
		),
		backups: response.backups.map((backup) =>
			toBackupQueueBackup(backup, activeByBackupId.get(backup.id)),
		),
	}
}

export function useServerBackupsQueue(
	serverId: Ref<string>,
	_worldId: Ref<string | null>,
) {
	const backend = injectHostingBackend()
	const queryClient = useQueryClient()
	const queryKey = computed(() => ['core-backups', 'queue', serverId.value] as const)

	const query = useQuery({
		queryKey,
		queryFn: async () => toBackupsQueueResponse(await backend.core.listBackups(serverId.value)),
		refetchInterval: (q) => {
			const data = q.state.data as CoreBackupsQueueResponse | undefined
			return data?.active_operations?.length ? 30_000 : false
		},
	})

	const data = computed(() => query.data.value)
	const activeOperations = computed(() => data.value?.active_operations ?? [])
	const backups = computed(() =>
		[...(data.value?.backups ?? [])].sort(
			(a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime(),
		),
	)

	const activeOperationByBackupId = computed(() => {
		const map = new Map<string, Archon.BackupsQueue.v1.ActiveOperation>()
		for (const op of activeOperations.value) map.set(op.backup_id, op)
		return map
	})
	const backupById = computed(() => {
		const map = new Map<string, Archon.BackupsQueue.v1.BackupQueueBackup>()
		for (const backup of backups.value) map.set(backup.id, backup)
		return map
	})

	const hasActiveCreate = computed(() =>
		activeOperations.value.some((operation) => operation.operation_type === 'create'),
	)
	const hasActiveRestore = computed(() =>
		activeOperations.value.some((operation) => operation.operation_type === 'restore'),
	)
	const hasRunningCreate = computed(() =>
		activeOperations.value.some(
			(operation) =>
				operation.operation_type === 'create' &&
				backupById.value.get(operation.backup_id)?.status === 'in_progress',
		),
	)
	const hasRunningRestore = computed(() =>
		activeOperations.value.some(
			(operation) =>
				operation.operation_type === 'restore' &&
				backupById.value.get(operation.backup_id)?.status === 'in_progress',
		),
	)

	function handleWsBackupProgress(_evt: Archon.Websocket.v0.WSBackupProgressEvent) {}

	function progressFor(_backupId: string, _kind: 'create' | 'restore'): number | undefined {
		return undefined
	}

	const busyReasons = computed<BusyReason[]>(() => {
		const reasons: BusyReason[] = []
		if (hasRunningCreate.value) {
			reasons.push({
				reason: defineMessage({
					id: 'servers.busy.backup-creating',
					defaultMessage: 'Backup creation in progress',
				}),
			})
		}
		if (hasRunningRestore.value) {
			reasons.push({
				reason: defineMessage({
					id: 'servers.busy.backup-restoring',
					defaultMessage: 'Backup restore in progress',
				}),
			})
		}
		return reasons
	})

	async function invalidate() {
		await queryClient.invalidateQueries({ queryKey: queryKey.value })
	}

	return {
		query,
		queryKey,
		data,
		activeOperations,
		activeOperationByBackupId,
		backups,
		hasActiveCreate,
		hasActiveRestore,
		hasRunningCreate,
		hasRunningRestore,
		progressFor,
		handleWsBackupProgress,
		busyReasons,
		invalidate,
	}
}
