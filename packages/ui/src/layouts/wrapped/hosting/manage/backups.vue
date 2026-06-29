<template>
	<Transition name="fade" mode="out-in">
		<div v-if="error" key="error" class="rounded-2xl bg-red-highlight p-4 font-semibold text-contrast">
			Failed to load backups: {{ error.message }}
		</div>

		<div v-else key="content" class="contents">
			<ReadyTransition :pending="backupsReadyPending">
				<BackupCreateModal ref="createModal" :backups="backupItems" />
				<BackupRenameModal ref="renameModal" :backups="backupItems" />
				<BackupRestoreModal ref="restoreModal" :can-restore="!backupRestoreDisabled" />
				<BackupDeleteModal ref="deleteModal" @delete="deleteBackup" @bulk-delete="bulkDelete" />

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
						<button @click="createModal?.show()">
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
							description="Create a backup before changing server files or content."
						>
							<template #actions>
								<ButtonStyled color="brand">
									<button class="mx-auto w-min" @click="createModal?.show()">
										<PlusIcon class="size-5" />
										Create backup
									</button>
								</ButtonStyled>
							</template>
						</EmptyState>
						<EmptyState
							v-else
							type="empty-inbox"
							heading="No backups match"
							description="Try a different filter or clear filters to see all backups."
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
									<CalendarIcon class="size-5" />
								</div>
								<span class="text-lg font-semibold leading-5 text-contrast">{{ group.label }}</span>
							</div>

							<TransitionGroup name="list" tag="div" class="flex flex-col">
								<div
									v-for="(backup, backupIndex) in group.backups"
									:key="`backup-${backup.id}`"
									class="flex gap-2"
								>
									<div class="flex w-5 flex-col items-center">
										<div
											class="w-px flex-1 bg-surface-5"
											:class="{ '-mt-1.5': backupIndex === 0 }"
										/>
										<Checkbox
											:model-value="selectedIds.has(backup.id)"
											:description="`Select backup ${backup.name}`"
											class="shrink-0"
											@update:model-value="toggleSelection(backup.id)"
										/>
										<div class="w-px flex-1 bg-surface-5" />
									</div>
									<BackupItem
										class="my-1.5 min-w-0 flex-1"
										:backup="backup"
										:selected="selectedIds.has(backup.id)"
										:restore-disabled="backupRestoreDisabledMessage"
										:show-copy-id-action="props.showCopyIdAction"
										:show-debug-info="props.showDebugInfo"
										@download="() => triggerDownloadAnimation()"
										@rename="() => renameModal?.show(backup)"
										@restore="() => showRestoreModal(backup)"
										@delete="() => deleteModal?.show(backup)"
									/>
								</div>
							</TransitionGroup>
						</template>
					</div>
				</div>

				<FloatingActionBar
					below-modal
					:shown="selectedIds.size > 0 || isBulkOperating"
					:aria-label="`${isBulkOperating ? bulkTotal : selectedIds.size} backups selected`"
				>
					<div class="flex items-center gap-0.5">
						<span class="px-4 py-2.5 text-base font-semibold tabular-nums text-contrast">
							{{ isBulkOperating ? bulkTotal : selectedIds.size }}
							{{ (isBulkOperating ? bulkTotal : selectedIds.size) === 1 ? 'backup selected' : 'backups selected' }}
						</span>
						<div class="mx-1 h-6 w-px bg-surface-5" />
						<ButtonStyled type="transparent">
							<button
								type="button"
								:disabled="isBulkOperating"
								:class="{ 'pointer-events-none opacity-60': isBulkOperating }"
								@click="deselectAll"
							>
								Clear
							</button>
						</ButtonStyled>
					</div>

					<div v-if="!isBulkOperating" class="ml-auto flex items-center gap-0.5">
						<ButtonStyled type="transparent" color="red" hover-color-fill="background">
							<button type="button" @click="confirmBulkDelete">
								<TrashIcon />
								<span class="bar-label">Delete</span>
							</button>
						</ButtonStyled>
					</div>

					<div v-else class="ml-auto flex items-center" aria-live="polite">
						<span class="px-4 py-2.5 text-base font-semibold tabular-nums text-secondary">
							Deleting {{ bulkTotal }} {{ bulkTotal === 1 ? 'backup' : 'backups' }}...
						</span>
					</div>

					<div v-if="isBulkOperating" class="absolute bottom-0 left-0 right-0 h-1">
						<div
							class="animate-indeterminate h-full rounded-l-full bg-brand"
							role="progressbar"
							aria-valuemin="0"
							:aria-valuemax="bulkTotal"
							style="box-shadow: 0px -2px 4px 0px rgba(27, 217, 106, 0.1)"
						/>
					</div>
				</FloatingActionBar>

				<div
					class="over-the-top-download-animation"
					:class="{ 'animation-hidden': !overTheTopDownloadAnimation }"
				>
					<div>
						<div
							class="animation-ring-3 flex items-center justify-center rounded-full border-4 border-solid border-brand bg-brand-highlight opacity-40"
						></div>
						<div
							class="animation-ring-2 flex items-center justify-center rounded-full border-4 border-solid border-brand bg-brand-highlight opacity-60"
						></div>
						<div
							class="animation-ring-1 flex items-center justify-center rounded-full border-4 border-solid border-brand bg-brand-highlight"
						>
							<DownloadIcon class="h-20 w-20 text-contrast" />
						</div>
					</div>
				</div>
			</ReadyTransition>
		</div>
	</Transition>
</template>

<script setup lang="ts">
import type { CoreBackup } from '@amberite/amberite-api'
import type { Archon } from '@modrinth/api-client'
import { CalendarIcon, DownloadIcon, PlusIcon, TrashIcon } from '@modrinth/assets'
import { useQuery } from '@tanstack/vue-query'
import { computed, ref } from 'vue'

import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import Checkbox from '#ui/components/base/Checkbox.vue'
import EmptyState from '#ui/components/base/EmptyState.vue'
import FilterPills from '#ui/components/base/FilterPills.vue'
import FloatingActionBar from '#ui/components/base/FloatingActionBar.vue'
import ReadyTransition from '#ui/components/base/ReadyTransition.vue'
import BackupCreateModal from '#ui/components/servers/backups/BackupCreateModal.vue'
import BackupDeleteModal from '#ui/components/servers/backups/BackupDeleteModal.vue'
import BackupItem from '#ui/components/servers/backups/BackupItem.vue'
import BackupRenameModal from '#ui/components/servers/backups/BackupRenameModal.vue'
import BackupRestoreModal from '#ui/components/servers/backups/BackupRestoreModal.vue'
import { useBackupsSelection } from '#ui/composables/hosting/backups-selection'
import { useBulkOperation } from '#ui/layouts/shared/content-tab/composables/bulk-operations'
import { injectHostingBackend, injectNotificationManager } from '#ui/providers'

import { injectCoreServerContext } from './context'
import { toBackupItem } from './mappers'

const props = defineProps<{
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
const overTheTopDownloadAnimation = ref(false)
const selectedFilters = ref<string[]>([])

const backupsQuery = useQuery({
	queryKey: computed(() => ['core-backups', ctx.instanceId.value]),
	queryFn: () => core.listBackups(ctx.instanceId.value),
	staleTime: 30_000,
})

const backups = computed<CoreBackup[]>(() => backupsQuery.data.value?.backups ?? [])
const activeOperations = computed(() => backupsQuery.data.value?.active_operations ?? [])
const error = computed(() => (backupsQuery.error.value as Error | null) ?? null)
const backupsReadyPending = computed(
	() => backupsQuery.isLoading.value && backupsQuery.data.value === undefined && !error.value,
)

const completedBackups = computed(() =>
	backups.value.filter((backup) => backup.status === 'done').map((backup) => toBackupItem(backup)),
)

const filteredBackups = computed(() => {
	if (selectedFilters.value.length === 0 || selectedFilters.value.length === 2) {
		return completedBackups.value
	}
	const wantAutomated = selectedFilters.value.includes('auto')
	return completedBackups.value.filter((backup) => backup.automated === wantAutomated)
})

const backupItems = computed(() => filteredBackups.value)

const groupedBackups = computed(() => {
	const now = Date.now()
	const groups: Array<{ label: string; backups: Archon.BackupsQueue.v1.BackupQueueBackup[] }> = []

	for (const backup of filteredBackups.value) {
		const created = new Date(backup.created_at).getTime()
		const diffMs = now - created
		const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24))
		let label = 'Older'

		if (diffMs < 30 * 60 * 1000) {
			label = 'Just now'
		} else if (diffDays === 0) {
			label = 'Earlier today'
		} else if (diffDays === 1) {
			label = 'Yesterday'
		} else if (diffDays <= 14) {
			label = 'Last 2 weeks'
		}

		let group = groups.find((entry) => entry.label === label)
		if (!group) {
			group = { label, backups: [] }
			groups.push(group)
		}
		group.backups.push(backup)
	}

	return groups
})

const {
	selectedIds,
	toggleSelection,
	deselectAll,
	toggleSelectAll,
	allSelected,
	someSelected,
	selectedBackups,
} = useBackupsSelection(filteredBackups, filteredBackups)

const { isBulkOperating, bulkTotal } = useBulkOperation()
const filterPillOptions = computed(() => [
	{ id: 'manual', label: 'Manual' },
	{ id: 'auto', label: 'Auto' },
])
const backupRestoreDisabled = computed(
	() =>
		!!props.isServerRunning ||
		activeOperations.value.some(
			(operation) => operation.operation_type === 'create' || operation.operation_type === 'restore',
		),
)
const backupRestoreDisabledMessage = computed(() => {
	if (props.isServerRunning) {
		return 'Cannot restore backup while server is running'
	}
	if (backupRestoreDisabled.value) {
		return 'A backup operation is already queued or in progress'
	}
	return undefined
})

function showRestoreModal(backup: Archon.BackupsQueue.v1.BackupQueueBackup) {
	if (backupRestoreDisabled.value) return
	restoreModal.value?.show(backup)
}

async function refresh() {
	await backupsQuery.refetch()
}

async function deleteBackup(backup: ReturnType<typeof toBackupItem> | undefined) {
	if (!backup) return
	await core.deleteBackup(ctx.instanceId.value, backup.id).catch(handleError)
	await refresh()
}

async function bulkDelete(selected: ReturnType<typeof toBackupItem>[]) {
	if (!selected.length) return
	isBulkOperating.value = true
	bulkTotal.value = selected.length

	try {
		await Promise.all(selected.map((backup) => core.deleteBackup(ctx.instanceId.value, backup.id))).catch(
			handleError,
		)
		await refresh()
	} finally {
		deselectAll()
		isBulkOperating.value = false
		bulkTotal.value = 0
	}
}

function confirmBulkDelete() {
	if (!selectedBackups.value.length) return
	deleteModal.value?.showBulk(selectedBackups.value)
}

function triggerDownloadAnimation() {
	overTheTopDownloadAnimation.value = true
	setTimeout(() => {
		overTheTopDownloadAnimation.value = false
	}, 500)
}
</script>

<style scoped>
.fade-enter-active,
.fade-leave-active {
	transition:
		opacity 300ms ease-in-out,
		transform 300ms ease-in-out;
}

.fade-enter-from,
.fade-leave-to {
	opacity: 0;
	transform: scale(0.98);
}

.list-enter-active,
.list-leave-active {
	transition: all 200ms ease-in-out;
}

.list-enter-from {
	opacity: 0;
	transform: translateY(-10px);
}

.list-leave-to {
	opacity: 0;
	transform: translateY(10px);
}

.list-move {
	transition: transform 200ms ease-in-out;
}

@keyframes indeterminate {
	0% {
		width: 20%;
		margin-left: -20%;
	}
	100% {
		width: 60%;
		margin-left: 100%;
	}
}

.animate-indeterminate {
	animation: indeterminate 1.5s ease-in-out infinite;
}

.over-the-top-download-animation {
	position: fixed;
	z-index: 100;
	inset: 0;
	display: flex;
	justify-content: center;
	align-items: center;
	pointer-events: none;
	scale: 0.5;
	transition: all 0.5s ease-out;
	opacity: 1;

	&.animation-hidden {
		scale: 0.8;
		opacity: 0;

		.animation-ring-1 {
			width: 25rem;
			height: 25rem;
		}
		.animation-ring-2 {
			width: 50rem;
			height: 50rem;
		}
		.animation-ring-3 {
			width: 100rem;
			height: 100rem;
		}
	}

	> div {
		position: relative;
		display: flex;
		justify-content: center;
		align-items: center;
		width: fit-content;
		height: fit-content;

		> * {
			position: absolute;
			scale: 1;
			transition: all 0.2s ease-out;
			width: 20rem;
			height: 20rem;
		}
	}
}
</style>
