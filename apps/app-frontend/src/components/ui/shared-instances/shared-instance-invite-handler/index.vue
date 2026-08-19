<template>
	<SharedInstanceInstallModal ref="installModal" />
	<SharedInstanceAlreadyInstalledModal
		ref="alreadyInstalledModal"
		@cancel="handleAlreadyInstalledCancel"
		@go-to-instance="handleAlreadyInstalledGoToInstance"
		@install-anyway="handleAlreadyInstalledInstallAnyway"
	/>
</template>

<script setup lang="ts">
import { makeFunctionReference } from 'convex/server'
import { onMounted, onUnmounted, ref } from 'vue'

import SharedInstanceInstallModal from '@/components/ui/shared-instances/shared-instance-install-modal/index.vue'
import SharedInstanceAlreadyInstalledModal from '@/components/ui/shared-instances/SharedInstanceAlreadyInstalledModal.vue'
import { useRealtimeConvexClient } from '@/composables/useSocialClient'

import type { AppNotification, SharedInstanceInviteHandler } from './shared-instance-invite-types'
import { useSharedInstanceInviteHandler } from './use-shared-instance-invite-handler'

const installModal = ref<InstanceType<typeof SharedInstanceInstallModal>>()
const alreadyInstalledModal = ref<InstanceType<typeof SharedInstanceAlreadyInstalledModal>>()
const {
	handleNotification,
	installFromInviteId,
	clearNotifications,
	handleAlreadyInstalledCancel,
	handleAlreadyInstalledGoToInstance,
	handleAlreadyInstalledInstallAnyway,
} = useSharedInstanceInviteHandler(installModal, alreadyInstalledModal)

const notificationsQuery = makeFunctionReference<'query', { userId?: string }, AppNotification[]>(
	'socialCompat:listNotifications',
)
let unsubscribeNotifications: (() => void) | undefined

onMounted(() => {
	unsubscribeNotifications = useRealtimeConvexClient().onUpdate(
		notificationsQuery,
		{},
		(notifications) => {
			for (const notification of notifications) void handleNotification(notification)
		},
		(error) => console.warn('Failed to subscribe to Amberite social notifications', error),
	)
})
onUnmounted(() => unsubscribeNotifications?.())

defineExpose<SharedInstanceInviteHandler>({
	handleNotification,
	installFromInviteId,
	clearNotifications,
})
</script>
