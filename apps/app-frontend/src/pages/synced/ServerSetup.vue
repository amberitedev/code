<template>
	<slot :is-server-running="isServerRunning" />
</template>

<script setup lang="ts">
import { useServerManageCoreRuntime } from '@modrinth/ui'
import { computed, onMounted, onUnmounted, ref } from 'vue'

const props = defineProps<{
	coreInstanceId: string
}>()

const isMounted = ref(true)
const isSyncingContent = ref(false)

const { isServerRunning, cleanupCoreRuntime, connectSocket } = useServerManageCoreRuntime({
	serverId: computed(() => props.coreInstanceId),
	worldId: ref(null),
	server: ref(null) as any,
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
