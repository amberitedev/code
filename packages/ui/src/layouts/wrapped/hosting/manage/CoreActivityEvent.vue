<template>
	<span class="min-w-0 truncate">
		<span class="font-semibold text-contrast">{{ label }}</span>
		<span v-if="details" class="text-secondary"> · {{ details }}</span>
	</span>
</template>

<script setup lang="ts">
defineProps<{
	label: string
	details?: string
}>()
</script>
