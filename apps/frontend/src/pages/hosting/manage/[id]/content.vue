<script setup lang="ts">
import {
	commonMessages,
	defineMessages,
	injectModrinthServerContext,
	ServersManageContentPage,
	useVIntl,
} from '@modrinth/ui'

const { server } = injectModrinthServerContext()
const { formatMessage } = useVIntl()
const messages = defineMessages({
	title: {
		id: 'servers.manage.content.title',
		defaultMessage: 'Content - {serverName} - Modrinth',
	},
})

useHead({
	title: () =>
		formatMessage(messages.title, {
			serverName: server.value?.name ?? formatMessage(commonMessages.serverLabel),
		}),
})
</script>

<template>
	<ServersManageContentPage :owner-avatar-url-base="''" />
</template>
