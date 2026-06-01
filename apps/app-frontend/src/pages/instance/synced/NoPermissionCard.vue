<template>
	<div class="flex items-center gap-3 rounded-2xl bg-bg-raised p-6 text-secondary">
		<LockIcon class="size-5 shrink-0" />
		<span class="text-sm font-medium">{{ label }}</span>
	</div>
</template>

<script setup lang="ts">
import { LockIcon } from '@modrinth/assets'

defineProps<{
	label: string
}>()
</script>
