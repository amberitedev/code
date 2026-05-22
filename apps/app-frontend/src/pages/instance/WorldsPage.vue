<template>
	<EmptyState
		v-if="instance.kind === 'server'"
		type="empty-inbox"
		heading="No client worlds"
		description="Dedicated Core servers do not have a local client worlds list. Use Files to manage the server world folder."
	/>
	<Worlds v-else v-bind="props" @play="emit('play')" @stop="emit('stop')" />
</template>

<script setup lang="ts">
import { EmptyState } from '@modrinth/ui'

import type { GameInstance } from '@/helpers/types'

import Worlds from './Worlds.vue'

const props = defineProps<{
	instance: GameInstance
	options: unknown
	offline: boolean
	playing: boolean
	installed: boolean
	openSettings?: () => void
}>()

const emit = defineEmits<{
	play: []
	stop: []
}>()
</script>
