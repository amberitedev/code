<template>
	<div class="grid gap-4 md:grid-cols-3">
		<div class="rounded-2xl bg-surface-2 p-4">
			<div class="text-sm font-semibold text-secondary">Game</div>
			<div class="mt-2 text-xl font-bold text-contrast capitalize">
				{{ instance.loader }} {{ instance.game_version }}
			</div>
		</div>
		<div class="rounded-2xl bg-surface-2 p-4">
			<div class="text-sm font-semibold text-secondary">Status</div>
			<div class="mt-2 text-xl font-bold text-contrast">
				{{ playing ? 'Running' : installed ? 'Installed' : 'Needs repair' }}
			</div>
		</div>
		<div class="rounded-2xl bg-surface-2 p-4">
			<div class="text-sm font-semibold text-secondary">Time played</div>
			<div class="mt-2 text-xl font-bold text-contrast">{{ timePlayed }} seconds</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

import type { GameInstance } from '@/helpers/types'

const props = defineProps<{
	instance: GameInstance
	playing: boolean
	installed: boolean
}>()

const timePlayed = computed(
	() => props.instance.recent_time_played + props.instance.submitted_time_played,
)
</script>
