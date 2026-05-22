<template>
	<div
		class="flex items-center gap-4 rounded-[20px] border border-solid bg-surface-3 p-4 shadow-[0px_1px_2px_0px_rgba(0,0,0,0.3),0px_1px_3px_0px_rgba(0,0,0,0.15)]"
		:class="selected ? 'border-brand-green' : 'border-transparent'"
	>
		<div class="flex min-w-0 flex-1 items-center gap-4">
			<div
				class="flex shrink-0 items-center justify-center rounded-2xl border border-solid border-surface-5 bg-surface-4"
				:class="preview ? 'size-10' : 'size-14'"
			>
				<component
					:is="backup.automated ? ShieldIcon : UserRoundIcon"
					class="text-secondary"
					:class="preview ? 'size-6' : 'size-10'"
				/>
			</div>

			<div class="flex min-w-0 flex-col gap-1.5">
				<div class="flex min-w-0 items-center gap-2">
					<span class="min-w-0 truncate font-semibold text-contrast">{{ backup.name }}</span>
					<span
						v-if="backup.automated"
						class="shrink-0 rounded-full border border-solid border-surface-5 bg-surface-4 px-2.5 py-1 text-sm font-medium text-secondary"
					>
						Auto
					</span>
					<span
						v-if="backup.locked"
						class="shrink-0 rounded-full border border-solid border-surface-5 bg-surface-4 px-2.5 py-1 text-sm font-medium text-secondary"
					>
						Locked
					</span>
				</div>
				<div class="flex items-center gap-1.5 text-sm font-medium text-secondary">
					<span>{{ subtitle }}</span>
					<span v-if="preview">· {{ formatBytes(backup.size_bytes, 1) }}</span>
				</div>
			</div>
		</div>

		<div v-if="!preview" class="flex shrink-0 items-center">
			<span class="whitespace-nowrap font-medium text-contrast">{{ formattedDate }}</span>
		</div>

		<div v-if="!preview" class="flex min-w-0 flex-1 items-center justify-end gap-2">
			<ButtonStyled color="brand" type="outlined">
				<button class="!border" :disabled="!!restoreDisabled" @click="emit('restore')">
					<RotateCounterClockwiseIcon class="size-5" />
					Restore
				</button>
			</ButtonStyled>
			<ButtonStyled circular type="transparent">
				<OverflowMenu :options="overflowMenuOptions">
					<MoreVerticalIcon class="size-5" />
					<template #copy-id> <ClipboardCopyIcon class="size-5" /> Copy ID </template>
					<template #rename> <EditIcon class="size-5" /> Rename </template>
					<template #lock>
						<LockIcon class="size-5" /> {{ backup.locked ? 'Unlock' : 'Lock' }}
					</template>
					<template #delete> <TrashIcon class="size-5" /> Delete </template>
				</OverflowMenu>
			</ButtonStyled>
		</div>
	</div>
</template>

<script setup lang="ts">
import type { CoreBackup } from '@amberite/amberite-api'
import {
	ClipboardCopyIcon,
	EditIcon,
	LockIcon,
	MoreVerticalIcon,
	RotateCounterClockwiseIcon,
	ShieldIcon,
	TrashIcon,
	UserRoundIcon,
} from '@modrinth/assets'
import { computed } from 'vue'

import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import OverflowMenu, { type Option as OverflowOption } from '#ui/components/base/OverflowMenu.vue'
import { useFormatBytes, useFormatDateTime } from '#ui/composables'

const props = withDefaults(
	defineProps<{
		backup: CoreBackup
		preview?: boolean
		restoreDisabled?: string
		selected?: boolean
		showCopyIdAction?: boolean
	}>(),
	{
		preview: false,
		restoreDisabled: undefined,
		selected: false,
		showCopyIdAction: false,
	},
)
const emit = defineEmits<{ restore: []; rename: []; delete: []; lock: []; copyId: [] }>()

const formatBytes = useFormatBytes()
const formatDateTime = useFormatDateTime({ timeStyle: 'short', dateStyle: 'long' })
const formattedDate = computed(() => formatDateTime(props.backup.created_at))
const subtitle = computed(() => {
	if (props.preview) return formattedDate.value
	return props.backup.automated ? 'Backup schedule' : 'Manual backup'
})
const overflowMenuOptions = computed<OverflowOption[]>(() => [
	{ id: 'copy-id', action: () => emit('copyId'), shown: props.showCopyIdAction },
	{ divider: true, shown: props.showCopyIdAction },
	{ id: 'rename', action: () => emit('rename') },
	{ id: 'lock', action: () => emit('lock') },
	{ divider: true },
	{ id: 'delete', color: 'danger', action: () => emit('delete') },
])
</script>
