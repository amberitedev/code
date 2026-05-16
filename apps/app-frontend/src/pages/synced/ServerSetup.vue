<template>
	<slot :is-server-running="isServerRunning" />
</template>

<script setup lang="ts">
import type { Archon } from '@modrinth/api-client'
import { useCoreServerManageRuntime } from '@modrinth/ui'
import type { Ref } from 'vue'
import { computed, onUnmounted, ref, watch } from 'vue'

const props = defineProps<{
	coreInstanceId: string
}>()

const isMounted = ref(true)
const isSyncingContent = ref(false)

const { isServerRunning, cleanupCoreRuntime, connectSocket } = useCoreServerManageRuntime({
	serverId: computed(() => props.coreInstanceId),
	worldId: ref(null),
	server: ref(null) as Ref<Archon.Servers.v0.Server | null>,
	isSyncingContent,
	incrementUptimeLocally: true,
	eventGuard: () => isMounted.value,
})

watch(
	() => props.coreInstanceId,
	(nextId, previousId) => {
		if (previousId && previousId !== nextId) cleanupCoreRuntime()
		if (nextId) void connectSocket(nextId)
	},
	{ immediate: true },
)

onUnmounted(() => {
	isMounted.value = false
	cleanupCoreRuntime()
})

defineExpose({ isServerRunning })
</script>
