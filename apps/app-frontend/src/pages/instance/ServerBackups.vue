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

		<div v-else class="flex flex-col gap-3">
			<BackupItem
				v-for="backup in backups"
				:key="backup.id"
				:backup="toBackupItem(backup)"
				@delete="() => deleteBackup(backup.id)"
				@restore="() => restoreBackup(backup.id)"
			/>
		</div>
	</div>
</template>

<script setup lang="ts">
import type { CoreBackup } from '@amberite/amberite-api'
import { DatabaseBackupIcon, PlusIcon } from '@modrinth/assets'
import { BackupItem, ButtonStyled, injectNotificationManager } from '@modrinth/ui'
import { inject, ref } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'

import { coreServerContextKey, toBackupItem } from './server/core-server-instance'

const core = useCoreClient()
const { handleError } = injectNotificationManager()
const ctx = inject(coreServerContextKey)
if (!ctx) throw new Error('Missing Core server context')

const backups = ref<CoreBackup[]>([])
const loading = ref(false)
const creating = ref(false)
const error = ref<Error | null>(null)

async function refresh() {
	loading.value = true
	error.value = null
	try {
		backups.value = (await core.listBackups(ctx.instanceId.value)).backups
	} catch (err) {
		error.value = err as Error
		handleError(err as Error)
	} finally {
		loading.value = false
	}
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

await refresh()
</script>
