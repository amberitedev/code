<script setup lang="ts">
import type { SharedInstances } from '@modrinth/api-client'
import { TrashIcon } from '@modrinth/assets'
import {
	ConfirmModal,
	IconButton,
	SegmentedBar,
	Table,
	type TableColumn,
	useFormatBytes,
	useRelativeTime,
} from '@modrinth/ui'
import { computed, ref } from 'vue'

import { useDeleteVersion, useSharedStorage } from '@/features/settings/sharing/use-shared-storage'

// The owner's view of one share's kept versions: what they take of the account's allowance, and
// a way to delete older ones. Settings for every share are under Settings, Sharing.

type StoredVersion = SharedInstances.Storage.v1.StoredVersion

const props = defineProps<{ shareId: string }>()

const formatBytes = useFormatBytes()
const formatRelativeTime = useRelativeTime()
const { data: storage } = useSharedStorage()
const deleteVersion = useDeleteVersion()

const share = computed(() => storage.value?.shares.find((each) => each.id === props.shareId))
const segments = computed(() => {
	if (!storage.value || !share.value) return []
	return [
		{ id: 'this', label: 'This instance', value: share.value.used, color: 'bg-brand' },
		{
			id: 'others',
			label: 'Other instances',
			value: storage.value.used - share.value.used,
			color: 'bg-gray',
		},
	].map((segment) => ({ ...segment, display: formatBytes(segment.value, 1) }))
})

const columns: TableColumn<'version' | 'created' | 'bytes' | 'actions'>[] = [
	{ key: 'version', label: 'Version', width: '7rem' },
	{ key: 'created', label: 'Created' },
	{ key: 'bytes', label: 'Size', width: '7rem' },
	{ key: 'actions', label: '', width: '5rem', align: 'right' },
]

const confirmDelete = ref<InstanceType<typeof ConfirmModal>>()
// Kept after the modal closes so its text does not change while it fades out.
const selected = ref<StoredVersion | null>(null)

function askDelete(version: StoredVersion) {
	selected.value = version
	confirmDelete.value?.show()
}
</script>

<template>
	<section
		v-if="storage && share"
		class="flex flex-col gap-3"
		aria-labelledby="share-versions-title"
	>
		<ConfirmModal
			ref="confirmDelete"
			:title="`Delete version ${selected?.version}?`"
			description="This can't be undone."
			proceed-label="Delete"
			@proceed="selected && deleteVersion.mutate({ share, version: selected })"
		/>
		<div class="flex items-baseline justify-between gap-4">
			<h2 id="share-versions-title" class="m-0 text-lg font-semibold text-contrast">Versions</h2>
			<span v-if="share.counted" class="font-semibold tabular-nums text-contrast">
				{{ formatBytes(share.used, 1) }} of {{ formatBytes(storage.allowance, 0) }}
			</span>
			<span v-else class="font-semibold tabular-nums text-contrast">
				{{ formatBytes(share.size, 1) }} on your Core
			</span>
		</div>
		<SegmentedBar v-if="share.counted" :segments="segments" :max="storage.allowance" />
		<Table :columns="columns" :data="share.versions" row-key="version">
			<template #cell-version="{ row }">
				<span class="font-semibold tabular-nums text-contrast">{{ row.version }}</span>
			</template>
			<template #cell-created="{ row }">
				{{ row.ready ? formatRelativeTime(row.created) : 'Uploading' }}
			</template>
			<template #cell-bytes="{ row }">
				<span class="tabular-nums">{{ formatBytes(row.bytes, 1) }}</span>
			</template>
			<template #cell-actions="{ row }">
				<div class="flex justify-end">
					<span v-if="row.current" class="text-contrast">Current</span>
					<IconButton
						v-else-if="row.ready"
						v-tooltip="'Delete version'"
						type="quiet"
						size="sm"
						:label="`Delete version ${row.version}`"
						:disabled="deleteVersion.isPending.value"
						@click="askDelete(row)"
					>
						<TrashIcon aria-hidden="true" />
					</IconButton>
				</div>
			</template>
		</Table>
	</section>
</template>
