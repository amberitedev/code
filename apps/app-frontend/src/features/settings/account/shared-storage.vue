<script setup lang="ts">
import type { SharedInstances } from '@modrinth/api-client'
import { TrashIcon, UpdatedIcon } from '@modrinth/assets'
import {
	Button,
	Checkbox,
	ConfirmModal,
	injectModrinthClient,
	injectNotificationManager,
	useFormatBytes,
	useRelativeTime,
} from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'

import { getErrorMessage } from '@/helpers/install'

// The account's shared storage: how much of its allowance on other people's Cores its shares
// use, how many versions each share keeps, and those versions with a way to delete them. It is
// the account's, not a Core's, so it shows whether or not the account has a Core.

type Share = SharedInstances.Storage.v1.StoredShare
type StoredVersion = SharedInstances.Storage.v1.StoredVersion

const DEFAULT_KEPT = 5
const MAX_KEPT = 1000

const client = injectModrinthClient()
const queryClient = useQueryClient()
const { addNotification } = injectNotificationManager()
const formatBytes = useFormatBytes()
const formatRelativeTime = useRelativeTime()

const queryKey = ['shared-storage']
const {
	data: storage,
	error,
	isFetching,
	refetch,
} = useQuery({
	queryKey,
	queryFn: () => client.sharedinstances.storage_v1.get(),
})

const overAllowance = computed(
	() => !!storage.value && storage.value.used > storage.value.allowance,
)
const usedPercent = computed(() => {
	const { used = 0, allowance = 0 } = storage.value ?? {}
	return allowance > 0 ? Math.min(used / allowance, 1) * 100 : 0
})

const changeKept = useMutation({
	mutationFn: (kept_versions: number | null) =>
		client.sharedinstances.storage_v1.change({ kept_versions }),
	onSuccess: (status) => queryClient.setQueryData(queryKey, status),
	onError: (cause) => {
		console.error('Failed to change kept versions', cause)
		resetKeptDraft()
		addNotification({
			type: 'error',
			title: 'Could not change kept versions',
			text: getErrorMessage(cause),
		})
	},
})

// What the number field shows. It follows the saved setting and resets when a change is cancelled.
const keptDraft = ref(DEFAULT_KEPT)
watch(
	() => storage.value?.kept_versions,
	(kept) => {
		keptDraft.value = kept ?? DEFAULT_KEPT
	},
	{ immediate: true },
)

const confirmKept = ref<InstanceType<typeof ConfirmModal> | null>(null)
// A lower setting waiting for confirmation, since it deletes versions at once. Kept after the
// modal closes so its text does not change while it fades out.
const pendingKept = ref<number | null>(null)

/** How many versions keeping only the last `kept` ready ones of each share would delete. */
function droppedBy(kept: number) {
	return (storage.value?.shares ?? []).reduce(
		(total, share) =>
			total + Math.max(share.versions.filter((version) => version.ready).length - kept, 0),
		0,
	)
}

function setKept(next: number | null) {
	if (!storage.value || next === storage.value.kept_versions) return
	if (next !== null && droppedBy(next) > 0) {
		pendingKept.value = next
		confirmKept.value?.show()
		return
	}
	changeKept.mutate(next)
}

function onKeptInput() {
	const value = Math.round(keptDraft.value)
	if (!Number.isFinite(value) || value < 1 || value > MAX_KEPT) return resetKeptDraft()
	keptDraft.value = value
	setKept(value)
}

// The modal hides before it emits proceed, so closing it always resets the field and proceeding
// sets it again right after.
function resetKeptDraft() {
	keptDraft.value = storage.value?.kept_versions ?? DEFAULT_KEPT
}

function proceedKept() {
	const next = pendingKept.value
	if (next === null) return
	keptDraft.value = next
	changeKept.mutate(next)
}

const confirmDelete = ref<InstanceType<typeof ConfirmModal> | null>(null)
const selected = ref<{ share: Share; version: StoredVersion } | null>(null)

const deleteVersion = useMutation({
	mutationFn: ({ share, version }: { share: Share; version: StoredVersion }) =>
		client.sharedinstances.instances_v1.deleteVersion(share.id, version.version),
	onError: (cause, { share, version }) => {
		console.error('Failed to delete version', cause)
		addNotification({
			type: 'error',
			title: `Could not delete version ${version.version} of ${share.name}`,
			text: getErrorMessage(cause),
		})
	},
	onSettled: () => queryClient.invalidateQueries({ queryKey }),
})

function askDelete(share: Share, version: StoredVersion) {
	selected.value = { share, version }
	confirmDelete.value?.show()
}

function proceedDelete() {
	if (selected.value) deleteVersion.mutate(selected.value)
}
</script>

<template>
	<section class="flex flex-col gap-4" aria-labelledby="shared-storage-title">
		<ConfirmModal
			ref="confirmKept"
			:title="`Delete ${pendingKept === null ? 0 : droppedBy(pendingKept)} old versions?`"
			:description="`Each shared instance keeps only its last ${pendingKept}. Older versions and their files are deleted now.`"
			proceed-label="Delete"
			:on-hide="resetKeptDraft"
			@proceed="proceedKept"
		/>
		<ConfirmModal
			ref="confirmDelete"
			:title="`Delete version ${selected?.version.version} of ${selected?.share.name}?`"
			description="Its files are deleted from storage. This can't be undone."
			proceed-label="Delete"
			@proceed="proceedDelete"
		/>
		<div class="flex flex-col gap-2">
			<div class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
				<h2 id="shared-storage-title" class="m-0 text-lg font-semibold text-contrast">
					Shared storage
				</h2>
				<span
					v-if="storage"
					class="font-semibold tabular-nums"
					:class="overAllowance ? 'text-red' : 'text-contrast'"
				>
					{{ formatBytes(storage.used, 1) }} of {{ formatBytes(storage.allowance, 0) }} used
				</span>
			</div>
			<div
				v-if="storage"
				role="meter"
				aria-labelledby="shared-storage-title"
				:aria-valuenow="storage.used"
				:aria-valuemin="0"
				:aria-valuemax="storage.allowance"
				class="h-3.5 overflow-hidden rounded-full bg-surface-4"
			>
				<div
					class="h-full bg-gradient-to-b from-white/10 to-black/10"
					:class="overAllowance ? 'bg-red' : 'bg-brand'"
					:style="{ width: `${usedPercent}%` }"
				/>
			</div>
		</div>

		<div v-if="error && !storage" class="flex items-center justify-between gap-4">
			<p class="m-0 text-red">
				Could not load your shared storage. Check your connection, then try again.
			</p>
			<Button :disabled="isFetching" @click="refetch()">
				<UpdatedIcon aria-hidden="true" /> Try again
			</Button>
		</div>
		<template v-else-if="storage">
			<div class="flex items-center justify-between gap-4">
				<label for="shared-storage-kept" class="font-medium text-contrast">Versions kept</label>
				<div class="flex items-center gap-4">
					<input
						id="shared-storage-kept"
						v-model.number="keptDraft"
						type="number"
						min="1"
						:max="MAX_KEPT"
						class="w-20 tabular-nums"
						:disabled="storage.kept_versions === null || changeKept.isPending.value"
						@change="onKeptInput"
					/>
					<Checkbox
						label="All"
						:model-value="storage.kept_versions === null"
						:disabled="changeKept.isPending.value"
						@update:model-value="(all) => setKept(all ? null : DEFAULT_KEPT)"
					/>
				</div>
			</div>

			<p v-if="!storage.shares.length" class="m-0 text-secondary">
				You haven't shared an instance yet.
			</p>
			<div v-else class="flex flex-col">
				<div
					v-for="share in storage.shares"
					:key="share.id"
					class="flex flex-col border-0 border-t border-solid border-surface-5 py-3"
				>
					<div class="flex items-baseline justify-between gap-4">
						<span class="truncate font-semibold text-contrast">{{ share.name }}</span>
						<span
							v-if="share.counted"
							class="shrink-0 text-sm font-semibold tabular-nums text-contrast"
						>
							{{ formatBytes(share.used, 1) }}
						</span>
						<span
							v-else
							v-tooltip="'Private or shared with no one, so it uses none of your storage'"
							class="shrink-0 text-sm text-secondary"
						>
							Not counted
						</span>
					</div>
					<ul class="m-0 flex list-none flex-col p-0">
						<li
							v-for="version in share.versions"
							:key="version.version"
							class="flex min-h-8 items-center justify-between gap-4 text-sm text-secondary"
						>
							<span class="truncate tabular-nums">
								Version {{ version.version }} · {{ formatRelativeTime(version.created) }} ·
								{{ formatBytes(version.bytes, 1) }}
								<template v-if="!version.ready"> · Uploading</template>
							</span>
							<span v-if="version.current" class="shrink-0 px-2 text-contrast">Current</span>
							<Button
								v-else
								v-tooltip="'Delete version'"
								type="quiet"
								size="sm"
								:aria-label="`Delete version ${version.version}`"
								:disabled="deleteVersion.isPending.value"
								@click="askDelete(share, version)"
							>
								<TrashIcon aria-hidden="true" />
							</Button>
						</li>
					</ul>
				</div>
			</div>
		</template>
	</section>
</template>
