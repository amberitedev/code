<template>
	<slot />
</template>

<script setup lang="ts">
import type { CoreInstanceSummary } from '@amberite/amberite-api'
import type { Archon } from '@modrinth/api-client'
import { useCoreServerManageRuntime } from '@modrinth/ui'
import type { Ref } from 'vue'
import { computed, onUnmounted, ref, watch } from 'vue'

const props = defineProps<{
	coreInstance: CoreInstanceSummary | null
	coreInstanceId: string
}>()

const isMounted = ref(true)
const isSyncingContent = ref(false)
const server = computed(() => {
	if (!props.coreInstanceId) return null
	return {
		server_id: props.coreInstanceId,
		name: props.coreInstance?.name ?? 'Server',
		owner_id: '',
		net: { ip: '127.0.0.1', port: props.coreInstance?.port ?? 25565, domain: null },
		game: 'java',
		backup_quota: 999,
		used_backup_quota: 0,
		status: props.coreInstance?.install_status === 'installing' ? 'installing' : 'available',
		suspension_reason: null,
		loader: props.coreInstance?.loader,
		loader_version: props.coreInstance?.loader_version ?? '',
		mc_version: props.coreInstance?.game_version,
		upstream: null,
		sftp_username: '',
		sftp_password: '',
		sftp_host: '',
		sftp_port: 22,
		datacenter: 'local',
		notices: [],
		node: { token: '', instance: '' },
		flows: { intro: false },
		is_medal: false,
	} as Archon.Servers.v0.Server
})

const { cleanupCoreRuntime, connectSocket } = useCoreServerManageRuntime({
	serverId: computed(() => props.coreInstanceId),
	worldId: ref(null),
	server: server as Ref<Archon.Servers.v0.Server | null>,
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
</script>
