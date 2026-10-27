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

// One Core's storage for the shared file pool: whether it holds other people's encrypted shared
// files, how much, and the caps its owner sets. Read from the Core itself, so it shows only while
// the Core answers. Turning it off never affects sharing.

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
				<span class="text-sm font-medium text-contrast">Store shared files</span>
				<span class="text-sm text-secondary">
					Holds other people's shared files, encrypted so you can't read them.
				</span>
			</label>
			<Toggle
				:id="`core-storage-${core.id}`"
				v-model="enabled"
				:disabled="change.isPending.value"
			/>
		</div>
		<div v-if="storage.enabled" class="flex flex-wrap items-center gap-x-6 gap-y-2 text-sm">
			<span class="tabular-nums text-secondary">
				{{ formatBytes(storage.used, 1) }} used · {{ formatBytes(storage.egress_today, 1) }} sent
				today
			</span>
			<label class="flex items-center gap-2 text-secondary">
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
			<label class="flex items-center gap-2 text-secondary">
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
