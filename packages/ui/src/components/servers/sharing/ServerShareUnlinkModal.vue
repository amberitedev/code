<template>
	<ConfirmUnlinkModal
		ref="modal"
		server
		:warning="{
			header: formatMessage(commonMessages.shareRequiresUnlinkHeader),
			body: formatMessage(commonMessages.shareRequiresUnlinkBody),
		}"
		:backup-tip="backupTip"
		hide-shift-click-hint
		:action-disabled="!!blocked"
		:action-disabled-tooltip="blocked ?? undefined"
	/>
</template>

<script setup lang="ts">
import type { Server } from '@amberite/contracts/core'
import { computed, ref } from 'vue'

import { defineMessages, useVIntl } from '#ui/composables/i18n'
import ConfirmUnlinkModal from '#ui/layouts/shared/content-tab/components/modals/ConfirmUnlinkModal.vue'
import { injectModrinthServerContext } from '#ui/providers'
import { commonMessages } from '#ui/utils/common-messages'

const { formatMessage } = useVIntl()
const messages = defineMessages({
	stopFirst: {
		id: 'servers.share.unlink.stop-first',
		defaultMessage: 'Stop the server to unlink its modpack',
	},
})
// The Core refuses to unlink while the server is busy or live. Setup has no server context: the
// invite waits for the install there.
const serverContext = injectModrinthServerContext(null)
const blocked = computed(() => {
	if (!serverContext) return null
	const busy = serverContext.busyReasons.value[0]
	if (busy) return formatMessage(busy.reason)
	return ['running', 'starting', 'stopping'].includes(serverContext.powerState.value)
		? formatMessage(messages.stopFirst)
		: null
})
const modal = ref<InstanceType<typeof ConfirmUnlinkModal>>()
const backupTip = ref<string>()

/** Asks to unlink the server's uploaded modpack before it is shared. Resolves true on confirm. */
async function ask(server: Server) {
	backupTip.value = server.uploaded_modpack?.name ?? server.name
	return (await modal.value?.ask()) ?? false
}

defineExpose({ ask })
</script>
