<script setup lang="ts">
import { UpdatedIcon } from '@modrinth/assets'
import {
	Button,
	ConfirmModal,
	injectAuth,
	injectModrinthClient,
	injectNotificationManager,
	SegmentedBar,
	Slider,
	Table,
	type TableColumn,
	useFormatBytes,
} from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { useDebounceFn } from '@vueuse/core'
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import AccountRequired from '@/features/cores/account-required.vue'
import { injectAppSettingsModal } from '@/features/settings/app-settings-modal/context'
import {
	countedShares,
	sharedStorageKey,
	useSharedStorage,
} from '@/features/settings/sharing/use-shared-storage'
import { getErrorMessage } from '@/helpers/install'
import { instanceListQueryOptions } from '@/pages/instance/query-options'

// The account's shared storage: how much of its allowance its shares use, each share's part of
// it, and how many versions they keep. Deleting single versions is on each instance's History tab.

/** The slider's top, which keeps every version. The backend stores that as null. */
const MAX_KEPT = 50

const client = injectModrinthClient()
const auth = injectAuth()
const queryClient = useQueryClient()
const { addNotification } = injectNotificationManager()
const formatBytes = useFormatBytes()

const { data: storage, error, isFetching, refetch } = useSharedStorage()
const overAllowance = computed(
	() => !!storage.value && storage.value.used > storage.value.allowance,
)

const counted = computed(() => (storage.value ? countedShares(storage.value) : []))
const segments = computed(() =>
	counted.value.map((share) => ({
		id: share.id,
		label: share.name,
		value: share.used,
		color: share.color,
		display: formatBytes(share.used, 1),
	})),
)

// The local instance of each share, to open its history from here.
const router = useRouter()
const settingsModal = injectAppSettingsModal(null)
const { data: instances } = useQuery(instanceListQueryOptions())
const instanceIds = computed(
	() =>
		new Map(
			(instances.value ?? []).flatMap((instance) =>
				instance.shared_instance ? [[instance.shared_instance.id, instance.id] as const] : [],
			),
		),
)
function openHistory(instanceId: string) {
	if (settingsModal && !settingsModal.close()) return
	void router.push(`/instance/${encodeURIComponent(instanceId)}/history`)
}

// Shares on someone else's Core first, in the bar's order, then ones only on the account's own.
const rows = computed(() => {
	const colors = new Map(counted.value.map((share) => [share.id, share.color]))
	return [...(storage.value?.shares ?? [])]
		.sort((a, b) => Number(b.counted) - Number(a.counted) || b.size - a.size)
		.map((share) => ({
			id: share.id,
			name: share.name,
			instanceId: instanceIds.value.get(share.id),
			color: colors.get(share.id),
			versions: share.versions.filter((version) => version.ready).length,
			size: share.size,
		}))
})
type Row = (typeof rows.value)[number]
const columns: TableColumn<keyof Row & string>[] = [
	{ key: 'name', label: 'Instance' },
	{ key: 'versions', label: 'Versions', width: '7rem' },
	{ key: 'size', label: 'Size', width: '7rem', align: 'right' },
]

// What the slider shows. It follows the saved setting and resets when a change fails.
const kept = ref(MAX_KEPT)
function resetKept() {
	kept.value = storage.value?.kept_versions ?? MAX_KEPT
}
watch(() => storage.value?.kept_versions, resetKept, { immediate: true })

const changeKept = useMutation({
	mutationFn: (next: number) =>
		client.sharedinstances.storage_v1.change({ kept_versions: next >= MAX_KEPT ? null : next }),
	onSuccess: (status) => queryClient.setQueryData(sharedStorageKey, status),
	onError: (cause) => {
		console.error('Failed to change kept versions', cause)
		resetKept()
		addNotification({
			type: 'error',
			title: 'Could not change kept versions',
			text: getErrorMessage(cause),
		})
	},
})

/** How many versions keeping only the last `count` ready ones of each share would delete. */
function droppedBy(count: number) {
	return (storage.value?.shares ?? []).reduce(
		(total, share) =>
			total + Math.max(share.versions.filter((version) => version.ready).length - count, 0),
		0,
	)
}

const confirmKept = ref<InstanceType<typeof ConfirmModal>>()
// The setting waiting for confirmation. Kept after the modal closes so its text does not change
// while it fades out.
const pending = ref(0)
const dropping = computed(() => droppedBy(pending.value))

// Saves once the slider rests, asking first when the new setting deletes versions.
const saveKept = useDebounceFn((next: number) => {
	if (next === (storage.value?.kept_versions ?? MAX_KEPT)) return
	if (next < MAX_KEPT && droppedBy(next) > 0) {
		pending.value = next
		confirmKept.value?.show()
	} else changeKept.mutate(next)
}, 600)

// Closing without confirming puts the slider back.
function onConfirmHidden() {
	if (!changeKept.isPending.value) resetKept()
}
</script>

<template>
	<AccountRequired v-if="!auth.user.value" description="Sign in to share instances" />
	<div v-else-if="error && !storage" class="flex items-center justify-between gap-4">
		<span class="text-red">Could not load your shared storage</span>
		<Button :disabled="isFetching" @click="refetch()">
			<UpdatedIcon aria-hidden="true" /> Try again
		</Button>
	</div>
	<div v-else-if="storage" class="flex flex-col gap-6">
		<ConfirmModal
			ref="confirmKept"
			:title="`Delete ${dropping} old ${dropping === 1 ? 'version' : 'versions'}?`"
			:description="`Each shared instance keeps only its last ${pending}.`"
			proceed-label="Delete"
			:on-hide="onConfirmHidden"
			@proceed="changeKept.mutate(pending)"
		/>
		<section class="flex flex-col gap-3" aria-labelledby="sharing-storage-title">
			<div class="flex items-baseline justify-between gap-4">
				<h2 id="sharing-storage-title" class="m-0 text-lg font-semibold text-contrast">Storage</h2>
				<span
					class="font-semibold tabular-nums"
					:class="overAllowance ? 'text-red' : 'text-contrast'"
				>
					{{ formatBytes(storage.used, 1) }} of {{ formatBytes(storage.allowance, 0) }}
				</span>
			</div>
			<SegmentedBar :segments="segments" :max="storage.allowance" />
		</section>

		<Table :columns="columns" :data="rows" row-key="id">
			<template #empty-state>
				<div class="flex h-24 items-center justify-center text-secondary">No shared instances</div>
			</template>
			<template #cell-name="{ row }">
				<span class="flex min-w-0 items-center gap-2">
					<span
						v-if="row.color"
						class="size-3 shrink-0 rounded-full"
						:class="row.color"
						aria-hidden="true"
					/>
					<span
						v-else
						v-tooltip="'Only on your Core'"
						class="size-3 shrink-0 rounded-full border-2 border-solid border-surface-5"
						aria-label="Only on your Core"
					/>
					<button
						v-if="row.instanceId"
						v-tooltip="'Open history'"
						class="min-w-0 cursor-pointer truncate border-none bg-transparent p-0 text-left font-semibold text-contrast hover:underline"
						@click="openHistory(row.instanceId)"
					>
						{{ row.name }}
					</button>
					<span v-else class="truncate font-semibold text-contrast">{{ row.name }}</span>
				</span>
			</template>
			<template #cell-versions="{ row }">
				<span class="tabular-nums">{{ row.versions }}</span>
			</template>
			<template #cell-size="{ row }">
				<span class="tabular-nums text-contrast">{{ formatBytes(row.size, 1) }}</span>
			</template>
		</Table>

		<section class="flex flex-col gap-2.5" aria-labelledby="sharing-kept-title">
			<h2 id="sharing-kept-title" class="m-0 text-lg font-semibold text-contrast">Versions kept</h2>
			<Slider
				v-model="kept"
				:min="1"
				:max="MAX_KEPT"
				:step="1"
				max-label="All"
				aria-label="Versions kept"
				:disabled="changeKept.isPending.value"
				@update:model-value="saveKept"
			/>
		</section>
	</div>
</template>
