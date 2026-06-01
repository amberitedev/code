<template>
	<div class="flex w-full flex-col gap-4">
		<div class="flex flex-wrap items-center justify-between gap-4">
			<h2 class="m-0 text-2xl font-semibold text-contrast">Backups</h2>
			<ButtonStyled color="brand">
				<button :disabled="loading || creating" @click="createBackup">
					<PlusIcon />
					{{ creating ? 'Creating backup...' : 'Create backup' }}
				</button>
			</ButtonStyled>
		</div>

		<div v-if="error" class="rounded-2xl bg-bg-red p-4 font-semibold text-contrast">
			Failed to load backups: {{ error.message }}
		</div>

		<div v-if="loading" class="rounded-[20px] bg-surface-3 p-6 font-semibold text-secondary">
			Loading backups...
		</div>

		<div
			v-else-if="backups.length === 0"
			class="mt-6 flex flex-col items-center justify-center gap-2 rounded-[20px] bg-surface-3 p-12 text-center text-secondary"
		>
			<DatabaseBackupIcon class="size-16" />
			<h3 class="m-0 text-2xl font-bold text-contrast">No backups yet</h3>
			<p class="m-0">Create a backup before changing server files or content.</p>
		</div>

		<div v-else-if="backupItems.length" class="flex flex-col gap-3">
			<BackupItem
				v-for="item in backupItems"
				:key="item.id"
				:backup="item"
				@delete="() => deleteBackup(item.id)"
				@restore="() => restoreBackup(item.id)"
			/>
		</div>
		<div
			v-else
			class="mt-6 flex flex-col items-center justify-center gap-2 rounded-[20px] bg-surface-3 p-12 text-center text-secondary"
		>
			<DatabaseBackupIcon class="size-16" />
			<h3 class="m-0 text-2xl font-bold text-contrast">No backups yet</h3>
			<p class="m-0">Create a backup before changing server files or content.</p>
		</div>
	</div>
</template>

<script setup lang="ts">
import type { CoreBackup } from '@amberite/amberite-api'
import { DatabaseBackupIcon, PlusIcon } from '@modrinth/assets'
import { BackupItem, ButtonStyled, injectNotificationManager } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, inject, ref } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'

import { coreServerContextKey, toBackupItem } from './server/core-server-instance'

const core = useCoreClient()
const { handleError } = injectNotificationManager()
const ctx = inject(coreServerContextKey)
if (!ctx) throw new Error('Missing Core server context')

const creating = ref(false)

const backupsQuery = useQuery({
	queryKey: computed(() => ['core-backups', ctx.instanceId.value]),
	queryFn: () => core.listBackups(ctx.instanceId.value).then((res) => res.backups),
	staleTime: 30_000,
})

const backups = computed<CoreBackup[]>(() => backupsQuery.data.value ?? [])
const loading = computed(() => backupsQuery.isLoading.value)
const error = computed(() => (backupsQuery.error.value as Error | null) ?? null)
const backupItems = computed(() => backups.value.map((b) => toBackupItem(b)))

async function refresh() {
	await backupsQuery.refetch()
}

async function createBackup() {
	creating.value = true
	try {
		await core.createBackup(ctx.instanceId.value)
		await refresh()
	} catch (err) {
		handleError(err as Error)
	} finally {
		creating.value = false
	}
}

async function deleteBackup(id: string) {
	await core.deleteBackup(ctx.instanceId.value, id).catch(handleError)
	await refresh()
}

async function restoreBackup(id: string) {
	await core.restoreBackup(ctx.instanceId.value, id).catch(handleError)
	await refresh()
}
</script>
