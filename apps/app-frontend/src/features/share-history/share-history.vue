<script setup lang="ts">
import { RotateCounterClockwiseIcon } from '@modrinth/assets'
import { AvatarGroup, ConfirmModal, useFormatDateTime, useRelativeTime } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, ref } from 'vue'

import { get_project_many } from '@/helpers/cache.js'
import type { SharedInstanceHistoryEntry } from '@/helpers/install'

import { changedItems, changeTitle } from './changes'
import VersionMeta from './version-meta.vue'
import VersionModal from './version-modal.vue'

// A share's history, newest first, each version named after what it changed over the one before
// it. Every History page shows it: an instance's and a server-first server's, where the content is
// managed (`editable`: restore and delete), and read-only on what follows them (a linked server,
// a Play instance). Where the versions come from is the page's; this only shows them.

const props = defineProps<{
	versions: SharedInstanceHistoryEntry[]
	loading: boolean
	editable: boolean
	busy: boolean
	/** What a restore changes, for its confirmation. */
	subject: 'instance' | 'server'
}>()

const emit = defineEmits<{ restore: [version: number]; delete: [version: number] }>()

const formatRelativeTime = useRelativeTime({ style: 'narrow' })
const formatDate = useFormatDateTime({ dateStyle: 'medium', timeStyle: 'short' })

// Icons of the projects the versions touched, from the App's project cache.
const projectIds = computed(() =>
	[
		...new Set(
			props.versions.flatMap((each) => each.diffs.flatMap((diff) => diff.projectId ?? [])),
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
	props.versions.map((version, index) => {
		/** The version the share started with, drawn as the start of the line. */
		const shared = version.version === 1
		return {
			...version,
			/** The newest version: the last push, which everyone has. */
			current: index === 0,
			title: shared ? 'Shared' : changeTitle(version.diffs, version.configFiles.length),
			items: changedItems(version.diffs, icons.value),
			shared,
		}
	}),
)
type Row = (typeof rows.value)[number]

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
	<div class="flex flex-col gap-4">
		<VersionModal
			ref="versionModal"
			:row="selectedRow"
			:editable="editable"
			:busy="busy"
			@restore="confirm(confirmRestore)"
			@delete="confirm(confirmDelete)"
		/>
		<template v-if="editable">
			<ConfirmModal
				ref="confirmRestore"
				title="Restore this version?"
				:description="`Changes to this ${subject} that aren't pushed are lost.`"
				proceed-label="Restore"
				:proceed-icon="RotateCounterClockwiseIcon"
				:danger="false"
				@proceed="selected && emit('restore', selected.version)"
			/>
			<ConfirmModal
				ref="confirmDelete"
				title="Delete this version?"
				description="This can't be undone."
				proceed-label="Delete"
				@proceed="selected && emit('delete', selected.version)"
			/>
		</template>

		<slot />

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
		<div v-else-if="!loading" class="text-secondary">No versions</div>
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
