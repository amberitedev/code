<template>
	<Transition name="fade" mode="out-in">
		<div
			v-if="error"
			key="error"
			class="flex w-full flex-col items-center justify-center gap-4 p-4"
		>
			<div class="flex max-w-lg flex-col items-center rounded-3xl bg-bg-raised p-6 shadow-xl">
				<div class="flex flex-col items-center text-center">
					<div class="flex flex-col items-center gap-4">
						<div class="grid place-content-center rounded-full bg-bg-orange p-4">
							<IssuesIcon class="size-12 text-orange" />
						</div>
						<h1 class="m-0 mb-2 w-fit text-4xl font-bold">Failed to load backups</h1>
					</div>
					<p class="text-lg text-secondary">
						We couldn't load your server's backups. Here's what went wrong:
					</p>
					<p>
						<span class="break-all font-mono">{{ error.message }}</span>
					</p>
					<ButtonStyled size="large" color="brand">
						<button class="mt-6 !w-full" @click="loadBackups">Retry</button>
					</ButtonStyled>
				</div>
			</div>
		</div>

		<div v-else key="content" class="contents">
			<ReadyTransition :pending="loading">
				<CoreBackupCreateModal
					ref="createBackupModal"
					:backups="completedBackups"
					:creating="busyAction === 'create'"
					@create="createBackup"
				/>
				<CoreBackupRenameModal
					ref="renameBackupModal"
					:saving="busyAction === 'rename'"
					@rename="renameBackup"
				/>
				<CoreBackupRestoreModal
					ref="restoreBackupModal"
					:is-server-running="isServerRunning"
					:restoring="busyAction === 'restore'"
					@restore="restoreBackup"
				/>
				<CoreBackupDeleteModal
					ref="deleteBackupModal"
					:deleting="busyAction === 'delete'"
					@delete="deleteBackup"
				/>

				<div
					v-if="completedBackups.length"
					class="mb-2 flex flex-wrap items-center justify-between gap-4"
				>
					<div class="flex min-w-0 flex-wrap items-center gap-4">
						<Checkbox
							:model-value="allSelected"
							:indeterminate="someSelected"
							label="Select all"
							class="shrink-0"
							label-class="text-secondary font-semibold"
							@update:model-value="toggleSelectAll"
						/>
						<div class="hidden h-6 w-px bg-surface-5 sm:block" />
						<FilterPills v-model="selectedFilters" :options="filterPillOptions">
							<template #all>All</template>
						</FilterPills>
					</div>
					<ButtonStyled color="brand">
						<button :disabled="busyAction !== null" @click="createBackupModal?.show()">
							<PlusIcon class="size-5" />
							Create backup
						</button>
					</ButtonStyled>
				</div>

				<div class="flex w-full flex-col gap-1.5">
					<div
						v-if="groupedBackups.length === 0"
						class="mt-6 flex flex-col items-center justify-center gap-2 text-center text-secondary"
					>
						<EmptyState
							v-if="completedBackups.length === 0"
							type="empty-inbox"
							heading="No backups yet"
							description="Create a backup before changing important server files."
						>
							<template #actions>
								<ButtonStyled color="brand">
									<button
										class="mx-auto w-min"
										:disabled="busyAction !== null"
										@click="createBackupModal?.show()"
									>
										<PlusIcon class="size-5" />
										Create backup
									</button>
								</ButtonStyled>
							</template>
						</EmptyState>
						<EmptyState
							v-else
							type="empty-inbox"
							heading="No backups match your filters"
							description="Clear your filters to show every backup."
						>
							<template #actions>
								<ButtonStyled type="outlined">
									<button @click="selectedFilters = []">Clear filters</button>
								</ButtonStyled>
							</template>
						</EmptyState>
					</div>

					<div v-else class="flex flex-col gap-3">
						<template v-for="group in groupedBackups" :key="group.label">
							<div class="flex items-center gap-2">
								<div class="flex w-5 shrink-0 items-center justify-center">
									<component :is="group.icon" class="size-5" />
								</div>
								<span class="text-lg font-semibold leading-5 text-contrast">{{ group.label }}</span>
							</div>

							<TransitionGroup name="list" tag="div" class="flex flex-col">
								<div
									v-for="(backup, backupIndex) in group.backups"
									:key="backup.id"
									class="flex gap-2"
								>
									<div class="flex w-5 flex-col items-center">
										<div
											class="w-px flex-1 bg-surface-5"
											:class="{ '-mt-1.5': backupIndex === 0 }"
										/>
										<Checkbox
											:model-value="selectedIds.has(backup.id)"
											:description="`Select ${backup.name}`"
											class="shrink-0"
											@update:model-value="toggleSelection(backup.id)"
										/>
										<div class="w-px flex-1 bg-surface-5" />
									</div>
									<CoreBackupItem
										class="my-1.5 min-w-0 flex-1"
										:backup="backup"
										:selected="selectedIds.has(backup.id)"
										:restore-disabled="backupRestoreDisabled"
										:show-copy-id-action="showCopyIdAction"
										@copy-id="copyBackupId(backup)"
										@rename="renameBackupModal?.show(backup)"
										@restore="restoreBackupModal?.show(backup)"
										@delete="deleteBackupModal?.show(backup)"
										@lock="lockBackup(backup)"
									/>
								</div>
							</TransitionGroup>
						</template>
					</div>
				</div>

				<FloatingActionBar
					below-modal
					:shown="selectedIds.size > 0 || busyAction === 'bulk-delete'"
					:aria-label="`${selectedIds.size} selected backups`"
				>
					<div class="flex items-center gap-0.5">
						<span class="px-4 py-2.5 text-base font-semibold tabular-nums text-contrast">
							{{ selectedIds.size }} selected
						</span>
						<div class="mx-1 h-6 w-px bg-surface-5" />
						<ButtonStyled type="transparent">
							<button type="button" :disabled="busyAction !== null" @click="deselectAll">
								Clear
							</button>
						</ButtonStyled>
					</div>

					<div class="ml-auto flex items-center gap-0.5">
						<ButtonStyled type="transparent" color="red" hover-color-fill="background">
							<button type="button" :disabled="busyAction !== null" @click="bulkDelete">
								<TrashIcon />
								<span class="bar-label">Delete</span>
							</button>
						</ButtonStyled>
					</div>
				</FloatingActionBar>
			</ReadyTransition>
		</div>
	</Transition>
</template>

<script setup lang="ts">
import type { CoreBackup } from '@amberite/amberite-api'
import { CalendarIcon, IssuesIcon, PlusIcon, TrashIcon } from '@modrinth/assets'
import dayjs from 'dayjs'
import type { Component } from 'vue'
import { computed, ref } from 'vue'

import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import Checkbox from '#ui/components/base/Checkbox.vue'
import EmptyState from '#ui/components/base/EmptyState.vue'
import FilterPills, { type FilterPillOption } from '#ui/components/base/FilterPills.vue'
import FloatingActionBar from '#ui/components/base/FloatingActionBar.vue'
import ReadyTransition from '#ui/components/base/ReadyTransition.vue'
import {
	injectCoreClient,
	injectModrinthServerContext,
	injectNotificationManager,
} from '#ui/providers'

import CoreBackupCreateModal from './components/CoreBackupCreateModal.vue'
import CoreBackupDeleteModal from './components/CoreBackupDeleteModal.vue'
import CoreBackupItem from './components/CoreBackupItem.vue'
import CoreBackupRenameModal from './components/CoreBackupRenameModal.vue'
import CoreBackupRestoreModal from './components/CoreBackupRestoreModal.vue'

withDefaults(defineProps<{ showCopyIdAction?: boolean }>(), {
	showCopyIdAction: false,
})

const coreClient = injectCoreClient()
const { addNotification } = injectNotificationManager()
const { serverId, isServerRunning } = injectModrinthServerContext()

const loading = ref(true)
const error = ref<Error | null>(null)
const backups = ref<CoreBackup[]>([])
const selectedIds = ref<Set<string>>(new Set())
const selectedFilters = ref<string[]>([])
const busyAction = ref<'create' | 'rename' | 'restore' | 'delete' | 'bulk-delete' | 'lock' | null>(
	null,
)
const createBackupModal = ref<InstanceType<typeof CoreBackupCreateModal>>()
const renameBackupModal = ref<InstanceType<typeof CoreBackupRenameModal>>()
const restoreBackupModal = ref<InstanceType<typeof CoreBackupRestoreModal>>()
const deleteBackupModal = ref<InstanceType<typeof CoreBackupDeleteModal>>()

const filterPillOptions: FilterPillOption[] = [
	{ id: 'manual', label: 'Manual' },
	{ id: 'automatic', label: 'Automatic' },
	{ id: 'locked', label: 'Locked' },
]

const completedBackups = computed(() => backups.value.filter((backup) => backup.status === 'done'))
const filteredBackups = computed(() => {
	if (selectedFilters.value.length === 0) return completedBackups.value
	return completedBackups.value.filter((backup) => {
		if (selectedFilters.value.includes('manual') && !backup.automated) return true
		if (selectedFilters.value.includes('automatic') && backup.automated) return true
		if (selectedFilters.value.includes('locked') && backup.locked) return true
		return false
	})
})
const groupedBackups = computed(() => groupBackups(filteredBackups.value))
const allSelected = computed(
	() =>
		filteredBackups.value.length > 0 &&
		filteredBackups.value.every((backup) => selectedIds.value.has(backup.id)),
)
const someSelected = computed(
	() =>
		filteredBackups.value.some((backup) => selectedIds.value.has(backup.id)) && !allSelected.value,
)
const backupRestoreDisabled = computed(() =>
	isServerRunning.value ? 'Stop the server before restoring a backup.' : undefined,
)

async function loadBackups() {
	loading.value = true
	error.value = null
	try {
		backups.value = (await coreClient.listBackups(serverId)).backups
	} catch (err) {
		error.value = err instanceof Error ? err : new Error(String(err))
	} finally {
		loading.value = false
	}
}

async function runAction(action: typeof busyAction.value, task: () => Promise<void>) {
	busyAction.value = action
	try {
		await task()
	} catch (err) {
		addNotification({
			type: 'error',
			title: 'Backup operation failed',
			text: err instanceof Error ? err.message : String(err),
		})
	} finally {
		busyAction.value = null
	}
}

async function createBackup(name: string) {
	await runAction('create', async () => {
		await coreClient.createBackup(serverId, name)
		createBackupModal.value?.hide()
		await loadBackups()
	})
}

async function renameBackup(backup: CoreBackup, name: string) {
	await runAction('rename', async () => {
		await coreClient.renameBackup(serverId, backup.id, name)
		renameBackupModal.value?.hide()
		await loadBackups()
	})
}

async function restoreBackup(backup: CoreBackup) {
	await runAction('restore', async () => {
		await coreClient.restoreBackup(serverId, backup.id)
		restoreBackupModal.value?.hide()
		await loadBackups()
	})
}

async function deleteBackup(backup: CoreBackup) {
	await runAction('delete', async () => {
		await coreClient.deleteBackup(serverId, backup.id)
		selectedIds.value.delete(backup.id)
		deleteBackupModal.value?.hide()
		await loadBackups()
	})
}

async function bulkDelete() {
	const ids = Array.from(selectedIds.value)
	if (ids.length === 0) return
	await runAction('bulk-delete', async () => {
		await coreClient.deleteManyBackups(serverId, ids)
		selectedIds.value.clear()
		await loadBackups()
	})
}

async function lockBackup(backup: CoreBackup) {
	await runAction('lock', async () => {
		await coreClient.lockBackup(serverId, backup.id, !backup.locked)
		await loadBackups()
	})
}

function toggleSelection(id: string) {
	const next = new Set(selectedIds.value)
	if (next.has(id)) next.delete(id)
	else next.add(id)
	selectedIds.value = next
}

function toggleSelectAll(value: boolean) {
	selectedIds.value = value ? new Set(filteredBackups.value.map((backup) => backup.id)) : new Set()
}

function deselectAll() {
	selectedIds.value = new Set()
}

async function copyBackupId(backup: CoreBackup) {
	await navigator.clipboard.writeText(backup.id)
}

type BackupGroup = {
	label: string
	icon: Component
	backups: CoreBackup[]
}

function groupBackups(source: CoreBackup[]) {
	const groups = new Map<string, BackupGroup>()
	for (const backup of [...source].sort(
		(a, b) => dayjs(b.created_at).valueOf() - dayjs(a.created_at).valueOf(),
	)) {
		const label = dayjs(backup.created_at).format('MMMM YYYY')
		const existing = groups.get(label)
		if (existing) existing.backups.push(backup)
		else groups.set(label, { label, icon: CalendarIcon, backups: [backup] })
	}
	return Array.from(groups.values())
}

await loadBackups()
</script>

<style scoped lang="scss">
.list-move,
.list-enter-active,
.list-leave-active {
	transition: all 0.2s ease;
}

.list-enter-from,
.list-leave-to {
	opacity: 0;
	transform: translateY(4px);
}
</style>
