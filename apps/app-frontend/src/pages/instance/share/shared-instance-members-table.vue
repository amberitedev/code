<template>
	<InvitedPlayersTable
		:rows="rows"
		:can-manage="!actionsLocked"
		:show-push-update="false"
		:invite-label="formatMessage(messages.inviteFriends)"
		:invite-disabled="inviteDisabled"
		:invite-pending="invitePending"
		@invite="management.invite($event)"
		@remove="management.remove"
	>
		<!-- One push button for friends and linked servers alike ("Push and restart"). -->
		<template #push-update>
			<InstancePushButton
				:instance="instance"
				:disabled="pushUpdateDisabled"
				@published="instancePage.refreshInstance"
			/>
		</template>
	</InvitedPlayersTable>
</template>

<script setup lang="ts">
import { defineMessages, InvitedPlayersTable, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import InstancePushButton from '@/features/servers/instance-push-button.vue'

import { injectInstancePage } from '../instance-context'
import { injectSharedInstanceManagement } from './shared-instance-management-context'

const management = injectSharedInstanceManagement()
const instancePage = injectInstancePage()
const instance = computed(() => instancePage.instance.value!)
const { formatMessage } = useVIntl()
const messages = defineMessages({
	inviteFriends: { id: 'app.instance.share.invite-friends', defaultMessage: 'Invite friends' },
})
const { rows, actionsLocked, inviteDisabled, invitePending, pushUpdateDisabled } = management
</script>
