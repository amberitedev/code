<script setup lang="ts">
import type { SharedInstances } from '@modrinth/api-client'
import { FileCogIcon, RotateCounterClockwiseIcon, TrashIcon } from '@modrinth/assets'
import { Avatar, Button, NewModal, useRelativeTime } from '@modrinth/ui'
import { ref } from 'vue'

import type { SharedInstanceVersionDetails } from '@/helpers/install'

import type { ChangedItem } from './changes'
import VersionMeta from './version-meta.vue'

// One kept version: what it runs on and the mods and config files it changed, framed as the App's
// other lists of changes are. The oldest version has nothing before it, so it lists what it holds;
// version 1 is what the instance was shared with. The owner can restore or delete any version but
// the current one, which is what members have.

defineProps<{
	row:
		| (SharedInstances.Storage.v1.StoredVersion & {
				/** Undefined while loading. */
				details?: SharedInstanceVersionDetails
				/** Undefined while loading. */
				title?: string
				items: ChangedItem[]
				oldest: boolean
				shared: boolean
		  })
		| null
	owner: boolean
	busy: boolean
}>()

const emit = defineEmits<{ restore: []; delete: [] }>()

const formatRelativeTime = useRelativeTime({ style: 'narrow' })

const fileName = (path: string) => path.slice(path.lastIndexOf('/') + 1)
const folder = (path: string) => path.slice(0, path.lastIndexOf('/') + 1)
const modal = ref<InstanceType<typeof NewModal>>()

defineExpose({
	show: (event?: MouseEvent) => modal.value?.show(event),
	hide: () => modal.value?.hide(),
})
</script>

<template>
	<NewModal ref="modal" max-width="544px" width="544px" no-padding scrollable>
		<template #title>
			<span v-if="row" class="flex min-w-0 items-baseline gap-3">
				<span class="truncate text-lg font-bold text-contrast">{{ row.title }}</span>
				<span class="shrink-0 text-secondary">
					{{ row.ready ? formatRelativeTime(row.created) : 'Uploading' }}
				</span>
			</span>
		</template>

		<template v-if="row?.details">
			<VersionMeta class="px-6 py-4" :details="row.details" :compare="!row.oldest" arrows />
			<div
				v-if="row.items.length || row.details.configFiles.length"
				class="flex flex-col border-0 border-y border-solid border-surface-5 bg-surface-2 px-4 py-2"
			>
				<div v-for="item in row.items" :key="item.key" class="flex h-12 items-center gap-3 px-2">
					<Avatar
						:src="item.src"
						:tint-by="item.name"
						size="2rem"
						pad-transparent-corners
						no-shadow
						:class="{ 'brightness-50 grayscale': item.muted }"
					/>
					<span
						class="min-w-0 flex-1 truncate font-semibold"
						:class="item.muted ? 'text-secondary' : 'text-contrast'"
					>
						{{ item.name }}
					</span>
					<span v-if="item.version" class="shrink-0 truncate text-sm text-secondary">
						{{ item.version }}
					</span>
				</div>
				<div
					v-for="path in row.details.configFiles"
					:key="path"
					class="flex h-12 items-center gap-3 px-2"
				>
					<span
						class="flex size-8 shrink-0 items-center justify-center rounded-[calc(16/96*2rem)] bg-button-bg text-secondary"
					>
						<FileCogIcon class="size-5" aria-hidden="true" />
					</span>
					<span class="min-w-0 flex-1 truncate font-semibold text-contrast">
						{{ fileName(path) }}
					</span>
					<span class="min-w-0 shrink truncate text-sm text-secondary">{{ folder(path) }}</span>
				</div>
			</div>
		</template>
		<div v-else class="h-24" aria-hidden="true" />

		<template v-if="owner && row?.ready && !row.current" #actions>
			<div class="flex justify-between gap-2">
				<Button type="quiet" color="red" :disabled="busy" @click="emit('delete')">
					<TrashIcon aria-hidden="true" /> Delete
				</Button>
				<Button type="colored" color="brand" :disabled="busy" @click="emit('restore')">
					<RotateCounterClockwiseIcon aria-hidden="true" /> Restore
				</Button>
			</div>
		</template>
	</NewModal>
</template>
