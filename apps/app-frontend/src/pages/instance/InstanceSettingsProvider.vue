<template>
	<slot />
</template>

<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { computed, ref, watch } from 'vue'

import { get_project_v3 } from '@/helpers/cache'
import type { GameInstance } from '@/helpers/types'
import { provideInstanceSettings } from '@/providers/instance-settings'

const props = defineProps<{
	instance: GameInstance
	offline?: boolean
}>()

const emit = defineEmits<{
	unlinked: []
}>()

const isMinecraftServer = ref(false)
const instanceRef = computed(() => props.instance)

provideInstanceSettings({
	instance: instanceRef,
	offline: props.offline,
	isMinecraftServer,
	onUnlinked: () => emit('unlinked'),
})

watch(
	() => props.instance,
	(instance) => {
		isMinecraftServer.value = false
		if (instance.linked_data?.project_id) {
			get_project_v3(instance.linked_data.project_id, 'must_revalidate')
				.then((project: Labrinth.Projects.v3.Project | undefined) => {
					if (project?.minecraft_server != null) isMinecraftServer.value = true
				})
				.catch(() => {})
		}
	},
	{ immediate: true },
)
</script>
