<template>
	<div class="flex flex-col gap-4">
		<div class="flex items-center justify-between gap-3">
			<h2 class="text-2xl font-semibold text-contrast">Backups</h2>
			<button
				class="rounded-full bg-green px-4 py-2 font-semibold text-white"
				@click="createBackup"
			>
				Create backup
			</button>
		</div>
		<div v-if="error" class="rounded-xl bg-red-highlight p-4 text-red">{{ error }}</div>
		<div class="overflow-hidden rounded-2xl bg-surface-2">
			<div
				v-for="backup in backups"
				:key="backup.id"
				class="flex flex-wrap items-center justify-between gap-3 border-b border-surface-5 px-4 py-3 last:border-b-0"
			>
				<div>
					<p class="font-semibold text-contrast">{{ backup.name }}</p>
					<p class="text-sm text-secondary">
						{{ backup.created_at }} · {{ formatBytes(backup.size_bytes, 1) }}
					</p>
				</div>
				<div class="flex gap-2">
					<button
						class="rounded-full bg-surface-4 px-3 py-1 text-sm font-semibold text-contrast"
						@click="restoreBackup(backup.id)"
					>
						Restore
					</button>
					<button
						class="rounded-full bg-red px-3 py-1 text-sm font-semibold text-white"
						@click="deleteBackup(backup.id)"
					>
						Delete
					</button>
				</div>
			</div>
			<p v-if="!backups.length" class="p-4 text-secondary">No backups yet.</p>
		</div>
	</div>
</template>

<script setup lang="ts">
import type { CoreBackup } from '@amberite/api-lib'
import { onMounted, ref } from 'vue'

import { useFormatBytes } from '#ui/composables/format-bytes'
import { injectCoreClient, injectModrinthServerContext } from '#ui/providers'

const coreClient = injectCoreClient()
const formatBytes = useFormatBytes()
const { serverId } = injectModrinthServerContext()
const backups = ref<CoreBackup[]>([])
const error = ref<string | null>(null)

async function loadBackups() {
	error.value = null
	try {
		backups.value = (await coreClient.listBackups(serverId)).backups
	} catch (loadError) {
		console.error('[core/server-manage] Failed to load backups:', loadError)
		error.value = 'Could not load backups.'
	}
}

async function createBackup() {
	await coreClient.createBackup(serverId)
	await loadBackups()
}

async function restoreBackup(backupId: string) {
	await coreClient.restoreBackup(serverId, backupId)
}

async function deleteBackup(backupId: string) {
	await coreClient.deleteBackup(serverId, backupId)
	await loadBackups()
}

onMounted(() => void loadBackups())
</script>
