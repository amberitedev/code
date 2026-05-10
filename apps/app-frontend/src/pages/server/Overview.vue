<template>
	<div class="flex flex-col gap-4">
		<div class="card p-4 flex flex-col gap-3">
			<h3 class="font-semibold text-lg">Server Details</h3>
			<div class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2 text-sm">
				<span class="text-secondary">Status</span>
				<span class="capitalize font-medium" :class="statusClass">{{ coreInstance.status }}</span>

				<span class="text-secondary">Game Version</span>
				<span>{{ coreInstance.game_version }}</span>

				<span class="text-secondary">Loader</span>
				<span class="capitalize">{{ coreInstance.loader }}</span>

				<template v-if="coreInstance.loader_version">
					<span class="text-secondary">Loader Version</span>
					<span>{{ coreInstance.loader_version }}</span>
				</template>

				<span class="text-secondary">Data Directory</span>
				<span class="font-mono text-xs break-all">{{ coreInstance.data_dir }}</span>
			</div>
		</div>

		<div class="card p-4 flex flex-col gap-3">
			<h3 class="font-semibold text-lg">Instance Info</h3>
			<div class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2 text-sm">
				<span class="text-secondary">Profile Name</span>
				<span>{{ instance.name }}</span>

				<span class="text-secondary">Core ID</span>
				<span class="font-mono text-xs">{{ coreInstance.id }}</span>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { CoreInstanceDetail } from '@/helpers/core'
import type { GameInstance } from '@/helpers/types'

const props = defineProps<{
	instance: GameInstance
	coreInstance: CoreInstanceDetail
}>()

const statusClass = computed(() => ({
	'text-green-400': props.coreInstance.status === 'running',
	'text-red-400': props.coreInstance.status === 'stopped',
	'text-yellow-400': ['starting', 'stopping'].includes(props.coreInstance.status),
}))
</script>
