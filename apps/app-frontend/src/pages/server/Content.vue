<template>
	<div class="flex flex-col gap-4">
		<p class="text-secondary">Mod management for server instances is coming soon.</p>
	</div>
</template>

<script setup lang="ts">
import type { CoreInstanceDetail } from '@/helpers/core'
import type { GameInstance } from '@/helpers/types'

defineProps<{
	instance: GameInstance
	coreInstance: CoreInstanceDetail
}>()
</script>
