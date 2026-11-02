<script setup lang="ts">
import { RotateCounterClockwiseIcon } from '@modrinth/assets'
import {
	AvatarGroup,
	ConfirmModal,
	injectNotificationManager,
	SegmentedBar,
	useFormatBytes,
	useFormatDateTime,
	useRelativeTime,
} from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'

import {
	countedShares,
	sharedHistorySettingsKey,
	sharedStorageKey,
	useDeleteVersion,
	useSharedStorage,
	versionHistoryKey,
} from '@/features/settings/sharing/use-shared-storage'
import { get_project_many } from '@/helpers/cache.js'
import {
	getErrorMessage,
	install_get_shared_instance_history,
	install_restore_shared_instance_version,
	wait_for_install_job,
} from '@/helpers/install'
import { injectAppEvents } from '@/providers/app-events'

import { injectInstancePage } from '../instance-context'
import { changedItems, changeTitle } from './changes'
import VersionMeta from './version-meta.vue'
import VersionModal from './version-modal.vue'

// The versions the owner pushed from this computer, newest first, each named after what it changed
// over the one before it. The owner also sees this share's part of the allowance and can restore a
// version into this instance, to push again, or delete one. Restoring reads only this computer.

const instancePage = injectInstancePage()
const appEvents = injectAppEvents()
const { addNotification } = injectNotificationManager()
const formatBytes = useFormatBytes()
const formatRelativeTime = useRelativeTime({ style: 'narrow' })
const formatDate = useFormatDateTime({ dateStyle: 'medium', timeStyle: 'short' })

const instance = computed(() => instancePage.instance.value!)
const shareId = computed(() => instance.value.shared_instance?.id)
const owner = computed(() => instance.value.shared_instance?.role === 'owner')

const history = useQuery({
	queryKey: computed(() => versionHistoryKey(instance.value.id)),
	queryFn: () => install_get_shared_instance_history(instance.value.id),
	enabled: () => !!shareId.value,
})
const versions = computed(() => history.data.value ?? [])

// A push adds a version and changes the share's usage.
const queryClient = useQueryClient()
watch(
	() => instance.value.shared_instance?.latest_version,
	() => {
		queryClient.invalidateQueries({ queryKey: versionHistoryKey(instance.value.id) })
		queryClient.invalidateQueries({ queryKey: sharedHistorySettingsKey })
		queryClient.invalidateQueries({ queryKey: sharedStorageKey })
	},
)

// Icons of the projects the versions touched, from the App's project cache.
const projectIds = computed(() =>
	[
		...new Set(
			versions.value.flatMap((each) => each.diffs.flatMap((diff) => diff.projectId ?? [])),
		),
	].sort(),
)
const projects = useQuery({
	queryKey: computed(() => ['shared-version-projects', projectIds.value] as const),
	queryFn: async (): Promise<{ id: string; icon_url?: string | null }[]> =>
		get_project_many(projectIds.value),
	enabled: () => projectIds.value.length > 0,
	staleTime: Infinity,
})
const icons = computed(
	() => new Map((projects.data.value ?? []).map((project) => [project.id, project.icon_url])),
)

const rows = computed(() =>
	versions.value.map((version, index) => {
		/** The version the instance was shared with, drawn as the start of the line. */
		const shared = version.version === 1
		return {
			...version,
			/** The newest version: the last push, which members have. */
			current: index === 0,
			title: shared ? 'Shared' : changeTitle(version.diffs, version.configFiles.length),
			items: changedItems(version.diffs, icons.value),
			shared,
		}
	}),
)
type Row = (typeof rows.value)[number]

const { data: storage } = useSharedStorage()
const share = computed(() => storage.value?.shares.find((each) => each.id === shareId.value))
const segments = computed(() => {
	if (!storage.value || !share.value) return []
	const color = countedShares(storage.value).find((each) => each.id === share.value!.id)?.color
	return [
		{ id: 'this', label: 'This instance', value: share.value.used, color: color ?? 'bg-brand' },
		{
			id: 'others',
			label: 'Other instances',
			value: storage.value.used - share.value.used,
			color: 'bg-gray',
		},
	].map((segment) => ({ ...segment, display: formatBytes(segment.value, 1) }))
})

const deleteVersion = useDeleteVersion()
const restore = useMutation({
	mutationFn: async (version: number) => {
		const job = await install_restore_shared_instance_version(instance.value.id, version)
		await wait_for_install_job(appEvents, job.job_id)
	},
	onSuccess: () => instancePage.refreshInstance(),
	onError: (cause) => {
		console.error('Failed to restore version', cause)
		addNotification({
			type: 'error',
			title: 'Could not restore the version',
			text: getErrorMessage(cause),
		})
	},
})
const busy = computed(
	() =>
		restore.isPending.value ||
		deleteVersion.isPending.value ||
		instance.value.install_stage !== 'installed',
)

const versionModal = ref<InstanceType<typeof VersionModal>>()
const confirmRestore = ref<InstanceType<typeof ConfirmModal>>()
const confirmDelete = ref<InstanceType<typeof ConfirmModal>>()
// Kept after a modal closes so its text does not change while it fades out.
const selected = ref<Row | null>(null)
// The row as it is now, since the history can reload while it is open.
const selectedRow = computed(
	() => rows.value.find((row) => row.version === selected.value?.version) ?? selected.value,
)

function open(row: Row, event: MouseEvent) {
	selected.value = row
	versionModal.value?.show(event)
}
function confirm(next?: InstanceType<typeof ConfirmModal>) {
	versionModal.value?.hide()
	next?.show()
}
</script>

<template>
	<div v-if="shareId" class="flex flex-col gap-4">
		<VersionModal
			ref="versionModal"
			:row="selectedRow"
			:owner="owner"
			:busy="busy"
			@restore="confirm(confirmRestore)"
			@delete="confirm(confirmDelete)"
		/>
		<ConfirmModal
			ref="confirmRestore"
			title="Restore this version?"
			description="Changes to this instance that aren't pushed are lost."
			proceed-label="Restore"
			:proceed-icon="RotateCounterClockwiseIcon"
			:danger="false"
			@proceed="selected && restore.mutate(selected.version)"
		/>
		<ConfirmModal
			ref="confirmDelete"
			title="Delete this version?"
			description="This can't be undone."
			proceed-label="Delete"
			@proceed="
				selected && deleteVersion.mutate({ instanceId: instance.id, version: selected.version })
			"
		/>

		<div v-if="owner && storage && share?.counted" class="flex items-center gap-4">
			<SegmentedBar class="flex-1" :segments="segments" :max="storage.allowance" />
			<span class="shrink-0 font-semibold tabular-nums text-contrast">
				{{ formatBytes(share.used, 1) }} / {{ formatBytes(storage.allowance, 0) }}
			</span>
		</div>

		<div v-if="rows.length" class="flex flex-col">
			<div v-for="(row, index) in rows" :key="row.version" class="relative pb-2">
				<div
					v-if="rows.length > 1"
					class="absolute left-[7px] w-0.5 bg-button-bg"
					:class="[
						index === 0 ? 'top-[calc(50%-4px)]' : 'top-0',
						index === rows.length - 1 ? 'bottom-[calc(50%+4px)]' : 'bottom-0',
					]"
					aria-hidden="true"
				/>
				<button
					class="version-row group relative flex w-full cursor-pointer items-center gap-4 border-none bg-transparent p-0 text-left text-primary"
					:aria-label="row.title ?? 'Version'"
					@click="open(row, $event)"
				>
					<span
						class="size-4 shrink-0 rounded-full transition-transform duration-150 group-hover:scale-125"
						:class="
							row.current
								? 'bg-brand'
								: row.shared
									? 'bg-bg shadow-[inset_0_0_0_2px_var(--color-button-bg)] group-hover:shadow-[inset_0_0_0_2px_var(--color-secondary)]'
									: 'bg-button-bg group-hover:bg-secondary'
						"
						aria-hidden="true"
					/>
					<span
						class="version-card flex min-h-20 min-w-0 flex-1 items-center gap-5 rounded-2xl px-5 py-3 group-hover:bg-button-bg"
						:class="row.shared && !row.current ? 'bg-transparent' : 'bg-bg-raised'"
					>
						<span class="flex min-w-0 flex-1 flex-col gap-1">
							<span class="truncate font-semibold text-contrast">{{ row.title }}</span>
							<VersionMeta class="text-sm" :details="row" />
						</span>
						<AvatarGroup class="w-2/5 shrink-0 self-stretch" :items="row.items" />
						<span
							v-tooltip="formatDate(row.created)"
							class="w-16 shrink-0 text-right tabular-nums text-contrast"
						>
							{{ formatRelativeTime(row.created) }}
						</span>
					</span>
				</button>
			</div>
		</div>
		<div v-else-if="!history.isPending.value" class="text-secondary">No versions</div>
	</div>
</template>

<style scoped>
/* Removed mods fade into the card behind them. */
.version-card {
	--avatar-group-bg: var(--surface-3);
}
.version-row:hover .version-card {
	--avatar-group-bg: var(--color-button-bg);
}
</style>
