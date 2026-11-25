import {
	type ActiveOperation,
	type Backup,
	type BackupTaskState,
	type ContractClient,
	core as coreRoutes,
	type CoreEvent,
} from '@modrinth/api-client'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, reactive } from 'vue'

import type { BusyReason } from '#ui/providers'

import { defineMessage } from './i18n'

type ProgressKey = `${string}:${'create' | 'restore'}`
type BackupProgress = Extract<CoreEvent, { type: 'backup_progress' }>['payload']

type ProgressStore = {
	overlay: Map<ProgressKey, number>
	lastSeenState: Map<ProgressKey, BackupTaskState>
}

const progressStores = new Map<string, ProgressStore>()

function progressStoreFor(serverId: string): ProgressStore {
	let store = progressStores.get(serverId)
	if (!store) {
		store = { overlay: reactive(new Map()), lastSeenState: new Map() }
		progressStores.set(serverId, store)
	}
	return store
}

export const serverBackupsQueueKey = (serverId: string) => ['backups', 'queue', serverId] as const

/** A server's backups on its Core, with the progress its socket reports while one runs. */
export function useServerBackupsQueue(serverId: string, core: ContractClient) {
	const queryClient = useQueryClient()
	const queryKey = serverBackupsQueueKey(serverId)
	const store = progressStoreFor(serverId)

	const query = useQuery({
		queryKey,
		queryFn: () => core.call(coreRoutes.ListBackups, { params: { id: serverId } }),
	})

	const data = computed(() => query.data.value)
	const options = computed(() => data.value?.options ?? null)
	const nextAutoBackupAt = computed(() => data.value?.next_auto_backup_at ?? null)
	const activeOperations = computed(() => data.value?.active_operations ?? [])
	const backups = computed(() => data.value?.backups ?? [])

	const activeOperationByBackupId = computed(() => {
		const map = new Map<string, ActiveOperation>()
		for (const op of activeOperations.value) map.set(op.backup_id, op)
		return map
	})
	const backupById = computed(() => {
		const map = new Map<string, Backup>()
		for (const backup of backups.value) map.set(backup.id, backup)
		return map
	})

	const running = (type: ActiveOperation['operation_type']) =>
		computed(() =>
			activeOperations.value.some(
				(o) =>
					o.operation_type === type && backupById.value.get(o.backup_id)?.status === 'in_progress',
			),
		)
	const hasActiveCreate = computed(() =>
		activeOperations.value.some((o) => o.operation_type === 'create'),
	)
	const hasActiveRestore = computed(() =>
		activeOperations.value.some((o) => o.operation_type === 'restore'),
	)
	const hasRunningCreate = running('create')
	const hasRunningRestore = running('restore')

	/** A `backup_progress` event from the Core's socket. */
	function handleBackupProgress(progress: BackupProgress) {
		const key = `${progress.id}:${progress.task}` as ProgressKey
		if (progress.state === 'ongoing') store.overlay.set(key, progress.progress)
		else store.overlay.delete(key)

		if (store.lastSeenState.get(key) !== progress.state) {
			store.lastSeenState.set(key, progress.state)
			void queryClient.invalidateQueries({ queryKey })
		}
	}

	function progressFor(backupId: string, kind: 'create' | 'restore'): number | undefined {
		return store.overlay.get(`${backupId}:${kind}`)
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
		await queryClient.invalidateQueries({ queryKey })
	}

	return {
		query,
		queryKey,
		data,
		options,
		nextAutoBackupAt,
		activeOperations,
		activeOperationByBackupId,
		backups,
		hasActiveCreate,
		hasActiveRestore,
		hasRunningCreate,
		hasRunningRestore,
		progressFor,
		handleBackupProgress,
		busyReasons,
		invalidate,
	}
}
