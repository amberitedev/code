<script setup lang="ts">
import { computed, ref, watchEffect } from 'vue'

import GridDisplay from '@/components/GridDisplay.vue'
import { get_project_v3_many } from '@/helpers/cache.js'

const props = defineProps({
	instances: {
		type: Array,
		required: true,
	},
})

const serverProjectIds = ref(new Set())

const typedServerInstances = computed(() =>
	props.instances.filter((i) => i.profile_type === 'server'),
)
const linkedInstances = computed(() =>
	props.instances.filter((i) => i.profile_type !== 'server' && i.linked_data),
)

watchEffect(async () => {
	const projectIds = [
		...new Set(linkedInstances.value.map((i) => i.linked_data?.project_id).filter(Boolean)),
	]
	if (projectIds.length === 0) {
		serverProjectIds.value = new Set()
		return
	}

	try {
		const projects = await get_project_v3_many(projectIds, 'must_revalidate')
		serverProjectIds.value = new Set(
			projects.filter((p) => p?.minecraft_server != null).map((p) => p.id),
		)
	} catch {
		serverProjectIds.value = new Set()
	}
})

const filteredInstances = computed(() => [
	...typedServerInstances.value,
	...linkedInstances.value.filter((i) => serverProjectIds.value.has(i.linked_data?.project_id)),
])
</script>
<template>
	<GridDisplay
		v-if="filteredInstances && filteredInstances.length > 0"
		label="Instances"
		:instances="filteredInstances"
	/>
</template>
