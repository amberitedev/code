<script setup lang="ts">
import type { Archon, Copal } from '@modrinth/api-client'
import {
	injectModrinthClient,
	injectNotificationManager,
	Toggle,
	useFormatBytes,
} from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed } from 'vue'

// One Core's storage for shared files: whether it offers storage to other people's shares, what it
// holds, and the caps its owner sets. Off makes the owner private: their shares live only on this
// Core and use no one else's storage. It always holds the owner's own shares, so its usage shows
// either way. Read from the Core itself, so it shows only while the Core answers.

const props = defineProps<{ core: Archon.Cores.v1.Core }>()

const client = injectModrinthClient()
const queryClient = useQueryClient()
const { addNotification } = injectNotificationManager()
const formatBytes = useFormatBytes()
const GB = 1024 ** 3

const queryKey = computed(() => ['core-storage', props.core.id])
const { data: storage } = useQuery({
	queryKey,
	queryFn: () => client.copal.storage_v1.get(props.core.id),
	staleTime: 30_000,
	retry: false,
})

const change = useMutation({
	mutationFn: (settings: Copal.StorageSettings) =>
		client.copal.storage_v1.change(props.core.id, settings),
	onSuccess: (status) => queryClient.setQueryData(queryKey.value, status),
	onError: (cause) => {
		console.error('Failed to change Core storage', cause)
		addNotification({
			type: 'error',
			title: `Could not change storage on ${props.core.name}`,
			text: 'Check that the Core is online, then try again.',
		})
	},
})

const enabled = computed({
	get: () => storage.value?.enabled ?? false,
	set: (value: boolean) => change.mutate({ enabled: value }),
})

/** A cap typed in whole gigabytes. Empty or invalid input changes nothing. */
function setCap(field: 'cap' | 'egress_cap', event: Event) {
	const value = Number((event.target as HTMLInputElement).value)
	if (!Number.isFinite(value) || value < 1) return
	const bytes = Math.round(value) * GB
	if (bytes !== storage.value?.[field]) change.mutate({ [field]: bytes })
}
</script>

<template>
	<div v-if="storage" class="flex flex-col gap-2 pb-3">
		<div class="flex items-center justify-between gap-4">
			<label :for="`core-storage-${core.id}`" class="flex min-w-0 flex-col gap-0.5">
				<span class="text-sm font-medium text-contrast">Store friends' shared files</span>
				<span class="text-sm text-secondary">
					{{
						storage.enabled
							? "Encrypted, so you can't read them."
							: "Private: your shares stay on this Core and use no one else's storage."
					}}
				</span>
			</label>
			<Toggle
				:id="`core-storage-${core.id}`"
				v-model="enabled"
				:disabled="change.isPending.value"
			/>
		</div>
		<div class="flex flex-wrap items-center gap-x-6 gap-y-2 text-sm">
			<span class="tabular-nums text-secondary">
				{{ formatBytes(storage.used, 1) }} used · {{ formatBytes(storage.egress_today, 1) }} sent
				today
			</span>
			<label v-if="storage.enabled" class="flex items-center gap-2 text-secondary">
				Disk cap
				<input
					type="number"
					min="1"
					class="w-20 tabular-nums"
					:value="Math.round(storage.cap / GB)"
					:disabled="change.isPending.value"
					@change="setCap('cap', $event)"
				/>
				GB
			</label>
			<label v-if="storage.enabled" class="flex items-center gap-2 text-secondary">
				Daily transfer cap
				<input
					type="number"
					min="1"
					class="w-20 tabular-nums"
					:value="Math.round(storage.egress_cap / GB)"
					:disabled="change.isPending.value"
					@change="setCap('egress_cap', $event)"
				/>
				GB
			</label>
		</div>
	</div>
</template>
