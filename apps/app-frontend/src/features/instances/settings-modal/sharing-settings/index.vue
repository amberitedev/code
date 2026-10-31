<script setup lang="ts">
import { SpinnerIcon, UnlinkIcon, UserPlusIcon } from '@modrinth/assets'
import {
	Admonition,
	Button,
	defineMessages,
	provideSharingSettings,
	SharingSettingsLayout,
	useVIntl,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { ref } from 'vue'

import { config } from '@/config'
import { create_shared_instance_invite_link } from '@/helpers/instance'
import { sharedInstanceErrorMessages } from '@/helpers/shared-instance-errors'
import { injectSharedInstance } from '@/pages/instance/shared-instance-context'

import { injectInstanceSettings } from '../instance-settings-context.ts'
import { useInstanceSharingSettings } from './use-sharing-settings'

provideSharingSettings(useInstanceSharingSettings())

const { instance, onUnlinked } = injectInstanceSettings()
const sharedInstance = injectSharedInstance()
const queryClient = useQueryClient()
const { formatMessage } = useVIntl()
const recovering = ref(false)
// Invite links open on the account website, which is the backend's when Amberite runs its own.
const shareSiteUrl = config.accountWebUrl || config.accountApiUrl || config.siteUrl

async function stopSharing() {
	recovering.value = true
	try {
		await sharedInstance.stopSharing()
		await queryClient.invalidateQueries({ queryKey: ['linkedModpackInfo', instance.value.id] })
		onUnlinked()
	} catch (error) {
		void sharedInstance.reportActionError(error)
	} finally {
		recovering.value = false
	}
}

/**
 * For a shared instance the backend no longer has. Detaching first makes creating an invite link
 * do what a first share does: create a fresh shared instance and publish to it.
 */
async function shareAgain() {
	const instanceId = instance.value.id
	recovering.value = true
	try {
		await sharedInstance.stopSharing()
		await create_shared_instance_invite_link(instanceId, { maxUses: 10 })
		await queryClient.invalidateQueries({ queryKey: ['sharedInstanceInvites', instanceId] })
	} catch (error) {
		void sharedInstance.reportActionError(error)
	} finally {
		recovering.value = false
	}
}

const messages = defineMessages({
	unpublishDescription: {
		id: 'installation-settings.shared-instance.unpublish-description',
		defaultMessage:
			'Remove this shared instance from Modrinth and stop sending updates to anyone using it. Your local instance will not be affected.',
	},
	unpublishConfirmation: {
		id: 'installation-settings.unpublish-shared-instance.modal.admonition-body',
		defaultMessage:
			"This deletes the shared instance from Modrinth's servers. People using it in the Modrinth App will stop receiving updates, but your local instance and its content will stay on this device.",
	},
})
</script>

<template>
	<div class="flex flex-col gap-8">
		<Admonition
			v-if="sharedInstance.missing.value"
			type="warning"
			:header="formatMessage(sharedInstanceErrorMessages.missingTitle)"
		>
			{{ formatMessage(sharedInstanceErrorMessages.missingText) }}
			<template #actions>
				<Button type="outlined" class="!border" :disabled="recovering" @click="stopSharing">
					<UnlinkIcon aria-hidden="true" />
					{{ formatMessage(sharedInstanceErrorMessages.stopSharingButton) }}
				</Button>
				<Button type="colored" color="brand" :disabled="recovering" @click="shareAgain">
					<SpinnerIcon v-if="recovering" class="animate-spin" aria-hidden="true" />
					<UserPlusIcon v-else aria-hidden="true" />
					{{ formatMessage(sharedInstanceErrorMessages.shareAgainButton) }}
				</Button>
			</template>
		</Admonition>
		<SharingSettingsLayout
			:site-url="shareSiteUrl"
			:unpublish-description="messages.unpublishDescription"
			:unpublish-confirmation="messages.unpublishConfirmation"
		/>
	</div>
</template>
