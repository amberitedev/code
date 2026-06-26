<template>
	<div class="flex w-full flex-col gap-4">
		<div class="flex flex-wrap items-center justify-between gap-4">
			<h2 class="m-0 text-2xl font-semibold text-contrast">Backups</h2>
			<ButtonStyled color="brand">
				<button :disabled="loading" @click="createModal?.show()">
					<PlusIcon />
					Create backup
				</button>
			</ButtonStyled>
		</div>

		<div v-if="error" class="rounded-2xl bg-red-highlight p-4 font-semibold text-contrast">
			Failed to load backups: {{ error.message }}
		</div>
		<div v-if="loading" class="rounded-2xl bg-surface-3 p-6 font-semibold text-secondary">
			Loading backups...
		</div>
		<div
			v-else-if="backupItems.length === 0"
			class="mt-6 flex flex-col items-center justify-center gap-2 rounded-2xl bg-surface-3 p-12 text-center text-secondary"
		>
			<DatabaseBackupIcon class="size-16" />
			<h3 class="m-0 text-2xl font-bold text-contrast">No backups yet</h3>
			<p class="m-0">Create a backup before changing server files or content.</p>
		</div>
		<div v-else class="flex flex-col gap-3">
			<BackupItem
				v-for="item in backupItems"
				:key="item.id"
				:backup="item"
				@delete="() => deleteModal?.show(item)"
				@rename="() => renameModal?.show(item)"
				@restore="() => restoreModal?.show(item)"
			/>
		</div>

		<BackupCreateModal ref="createModal" :backups="backupItems" />
		<BackupDeleteModal
			ref="deleteModal"
			@delete="deleteBackup"
			@bulk-delete="deleteBackups"
		/>
		<BackupRenameModal ref="renameModal" :backups="backupItems" />
		<BackupRestoreModal ref="restoreModal" />
	</div>
</template>

<script setup lang="ts">
import type { CoreBackup } from '@amberite/amberite-api'
import { DatabaseBackupIcon, PlusIcon } from '@modrinth/assets'
import { useQuery } from '@tanstack/vue-query'
import { computed, ref } from 'vue'

import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import BackupCreateModal from '#ui/components/servers/backups/BackupCreateModal.vue'
import BackupDeleteModal from '#ui/components/servers/backups/BackupDeleteModal.vue'
import BackupItem from '#ui/components/servers/backups/BackupItem.vue'
import BackupRenameModal from '#ui/components/servers/backups/BackupRenameModal.vue'
import BackupRestoreModal from '#ui/components/servers/backups/BackupRestoreModal.vue'
import { injectHostingBackend, injectNotificationManager } from '#ui/providers'

import { injectCoreServerContext } from './context'
import { toBackupItem } from './mappers'

defineProps<{
	isServerRunning?: boolean
	showCopyIdAction?: boolean
	showDebugInfo?: boolean
}>()

const backend = injectHostingBackend()
const core = backend.core
const { handleError } = injectNotificationManager()
const ctx = injectCoreServerContext()
const createModal = ref<InstanceType<typeof BackupCreateModal>>()
const deleteModal = ref<InstanceType<typeof BackupDeleteModal>>()
const renameModal = ref<InstanceType<typeof BackupRenameModal>>()
const restoreModal = ref<InstanceType<typeof BackupRestoreModal>>()
const backupsQuery = useQuery({
	queryKey: computed(() => ['core-backups', ctx.instanceId.value]),
	queryFn: () => core.listBackups(ctx.instanceId.value).then((res) => res.backups),
	staleTime: 30_000,
})

const backups = computed<CoreBackup[]>(() => backupsQuery.data.value ?? [])
const loading = computed(() => backupsQuery.isLoading.value)
const error = computed(() => (backupsQuery.error.value as Error | null) ?? null)
const backupItems = computed(() => backups.value.map((backup) => toBackupItem(backup)))

async function refresh() {
	await backupsQuery.refetch()
}

async function deleteBackup(backup: ReturnType<typeof toBackupItem> | undefined) {
	if (!backup) return
	await core.deleteBackup(ctx.instanceId.value, backup.id).catch(handleError)
	await refresh()
}

async function deleteBackups(selectedBackups: ReturnType<typeof toBackupItem>[]) {
	await Promise.all(
		selectedBackups.map((backup) => core.deleteBackup(ctx.instanceId.value, backup.id)),
	).catch(handleError)
	await refresh()
}
</script>
