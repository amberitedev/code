<script setup lang="ts">
import { defineMessages, provideSharingSettings, SharingSettingsLayout } from '@modrinth/ui'

import { config } from '@/config'

import { useInstanceSharingSettings } from './use-sharing-settings'

provideSharingSettings(useInstanceSharingSettings())

const messages = defineMessages({
	unpublishDescription: {
		id: 'installation-settings.shared-instance.unpublish-description',
		defaultMessage:
			'Remove this shared instance from Modrinth and stop sending updates to anyone using it. Your local instance will not be affected.',
	},
	unpublishConfirmation: {
		id: 'installation-settings.unpublish-shared-instance.modal.admonition-body',
		defaultMessage:
			'This stops sharing the instance. People using it will stop receiving updates, but your local instance and its content will stay on this device.',
	},
})
</script>

<template>
	<SharingSettingsLayout
		:site-url="config.siteUrl"
		:unpublish-description="messages.unpublishDescription"
		:unpublish-confirmation="messages.unpublishConfirmation"
	/>
</template>
