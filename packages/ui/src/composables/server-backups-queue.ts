import type { CoreBackup, CoreBackupOperation, CoreBackupsResponse } from '@amberite/core-client'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, type Ref } from 'vue'

import { type BusyReason, injectCoreClient } from '#ui/providers'

import { defineMessage } from './i18n'

export function useServerBackupsQueue(serverId: Ref<string>, _worldId?: Ref<string | null>) {
	const coreClient = injectCoreClient()
	const queryClient = useQueryClient()

	const queryKey = computed(() => ['backups', 'queue', serverId.value] as const)

	const query = useQuery({
		queryKey,
		queryFn: (): Promise<CoreBackupsResponse> => coreClient.listBackups(serverId.value),
		enabled: computed(() => !!serverId.value),
		refetchInterval: (q) => {
			const data = q.state.data as CoreBackupsResponse | undefined
			return data?.active_operations?.length ? 3000 : false
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
		const map = new Map<string, CoreBackupOperation>()
		for (const op of activeOperations.value) map.set(op.backup_id, op)
		return map
	})
	const backupById = computed(() => {
		const map = new Map<string, CoreBackup>()
		for (const b of backups.value) map.set(b.id, b)
		return map
	})

	const hasActiveCreate = computed(() =>
		activeOperations.value.some((o) => o.operation_type === 'create'),
	)
	const hasActiveRestore = computed(() =>
		activeOperations.value.some((o) => o.operation_type === 'restore'),
	)
	const hasRunningCreate = computed(() =>
		activeOperations.value.some(
			(o) => o.operation_type === 'create' && backupById.value.get(o.backup_id)?.status === 'in_progress',
		),
	)
	const hasRunningRestore = computed(() =>
		activeOperations.value.some(
			(o) => o.operation_type === 'restore' && backupById.value.get(o.backup_id)?.status === 'in_progress',
		),
	)

	/** No-op — Core backups don't emit WS progress events. */
	function handleWsBackupProgress(_evt: unknown) {}

	/** Always undefined — progress overlay not supported for Core backups. */
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
