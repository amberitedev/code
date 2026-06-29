<template>
	<BaseEvent>
		<span class="font-semibold text-contrast">{{ label }}</span>
		<span v-if="details" class="text-secondary"> · {{ details }}</span>
	</BaseEvent>
</template>

<script setup lang="ts">
import BaseEvent from '#ui/components/servers/access/events/BaseEvent.vue'

defineProps<{
	label: string
	details?: string
}>()
</script>
