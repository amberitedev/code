<template>
	<div class="rounded-2xl bg-surface-2 p-4">
		<div class="flex items-center justify-between">
			<h3 class="m-0 text-lg font-bold text-contrast">Status</h3>
			<span
				class="rounded-full px-2 py-0.5 text-xs font-semibold"
				:class="
					query.data.value?.online ? 'bg-green-500/20 text-green-500' : 'bg-red-500/20 text-red-500'
				"
			>
				{{ query.data.value?.online ? 'Online' : 'Offline' }}
			</span>
		</div>

		<div v-if="query.isLoading.value && !query.data.value" class="mt-3 text-sm text-secondary">
			Querying server...
		</div>

		<div v-else-if="query.error.value" class="mt-3 text-sm text-red-500">
			Unable to query server: {{ query.error.value.message }}
		</div>

		<div v-else-if="!query.data.value?.online" class="mt-3 text-sm text-secondary">
			Server is not accepting connections. Start the server to see live status.
		</div>

		<div v-else class="mt-3 flex flex-col gap-2 text-sm">
			<div class="flex items-center gap-2">
				<span class="text-secondary">Version:</span>
				<span class="font-medium text-contrast">{{ query.data.value.version_name }}</span>
			</div>
			<div class="flex items-center gap-2">
				<span class="text-secondary">Players:</span>
				<span class="font-medium text-contrast">
					{{ query.data.value.players_online }} / {{ query.data.value.players_max }}
				</span>
			</div>
			<div class="flex items-center gap-2">
				<span class="text-secondary">Latency:</span>
				<span class="font-medium text-contrast">{{ query.data.value.latency_ms }} ms</span>
			</div>
			<div v-if="query.data.value.motd" class="mt-1">
				<span class="text-secondary">MOTD:</span>
				<p class="m-0 mt-0.5 whitespace-pre-wrap font-medium text-contrast">
					{{ query.data.value.motd }}
				</p>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import type { CoreServerQuery } from '@modrinth/api-client'
import { useQuery } from '@tanstack/vue-query'
import { computed } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'

const props = defineProps<{
	instanceId: string
	enabled: boolean
}>()

const core = useCoreClient()

const query = useQuery({
	queryKey: computed(() => ['core-query', props.instanceId]),
	queryFn: (): Promise<CoreServerQuery> => core.queryInstance(props.instanceId),
	staleTime: 30_000,
	refetchInterval: 30_000,
	enabled: computed(() => props.enabled),
})
</script>
