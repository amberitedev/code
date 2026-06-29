<template>
	<div class="rounded-2xl border border-solid border-surface-5 bg-bg-raised p-4">
		<div class="flex items-center justify-between gap-3">
			<h3 class="m-0 text-lg font-bold text-contrast">Status</h3>
			<span
				class="rounded-full border border-solid px-2.5 py-1 text-xs font-semibold"
				:class="query.data.value?.online ? 'border-green bg-green-highlight text-green' : 'border-surface-5 bg-surface-4 text-primary'"
			>
				{{ query.data.value?.online ? 'Online' : 'Offline' }}
			</span>
		</div>

		<div v-if="query.isLoading.value && !query.data.value" class="mt-3 text-sm text-secondary">
			Querying server...
		</div>
		<div v-else-if="query.error.value" class="mt-3 text-sm text-red">
			Unable to query server: {{ query.error.value.message }}
		</div>
		<div v-else-if="!query.data.value?.online" class="mt-3 text-sm text-secondary">
			Server is not accepting connections. Start the server to see live status.
		</div>
		<div v-else class="mt-3 flex flex-col gap-3 text-sm">
			<div class="flex flex-wrap gap-2">
				<div class="rounded-full bg-surface-4 px-2.5 py-1 font-medium text-primary">
					Version {{ query.data.value.version_name }}
				</div>
				<div class="rounded-full bg-surface-4 px-2.5 py-1 font-medium text-primary">
					{{ query.data.value.players_online }} / {{ query.data.value.players_max }} players
				</div>
				<div class="rounded-full bg-surface-4 px-2.5 py-1 font-medium text-primary">
					{{ query.data.value.latency_ms }} ms
				</div>
			</div>

			<div v-if="query.data.value.sample.length > 0" class="flex flex-col gap-1">
				<span class="text-secondary">Online players</span>
				<p class="m-0 text-primary">
					{{ query.data.value.sample.join(', ') }}
				</p>
			</div>

			<div v-if="query.data.value.motd" class="flex flex-col gap-1">
				<span class="text-secondary">MOTD</span>
				<p class="m-0 whitespace-pre-wrap font-medium text-contrast">
					{{ query.data.value.motd }}
				</p>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import type { CoreServerQuery } from '@amberite/amberite-api'
import { useQuery } from '@tanstack/vue-query'
import { computed } from 'vue'

import { injectHostingBackend } from '#ui/providers'

const props = defineProps<{
	instanceId: string
	enabled: boolean
}>()

const backend = injectHostingBackend()
const query = useQuery({
	queryKey: computed(() => ['core-query', props.instanceId]),
	queryFn: (): Promise<CoreServerQuery> => backend.core.queryInstance(props.instanceId),
	staleTime: 30_000,
	refetchInterval: 30_000,
	enabled: computed(() => props.enabled),
})
</script>
