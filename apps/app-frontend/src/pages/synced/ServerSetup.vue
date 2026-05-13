<template>
	<slot :is-server-running="isServerRunning" />
</template>

<script setup lang="ts">
import type { Archon } from '@modrinth/api-client'
import { useServerManageCoreRuntime } from '@modrinth/ui'
import type { Ref } from 'vue'
import { computed, onMounted, onUnmounted, ref } from 'vue'

const props = defineProps<{
	coreInstanceId: string
}>()

const isMounted = ref(true)
const isSyncingContent = ref(false)

const { isServerRunning, cleanupCoreRuntime, connectSocket } = useServerManageCoreRuntime({
	serverId: computed(() => props.coreInstanceId),
	worldId: ref(null),
	server: ref(null) as Ref<Archon.Servers.v0.Server | null>,
	isSyncingContent,
	incrementUptimeLocally: true,
	eventGuard: () => isMounted.value,
})

onMounted(() => connectSocket(props.coreInstanceId))
onUnmounted(() => {
	isMounted.value = false
	cleanupCoreRuntime(props.coreInstanceId)
})

defineExpose({ isServerRunning })
</script>
