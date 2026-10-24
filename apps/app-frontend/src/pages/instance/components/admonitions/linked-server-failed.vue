<template>
	<Admonition
		type="critical"
		inline-actions
		:header="formatMessage(messages.linkedServerFailedHeader)"
	>
		{{
			formatMessage(messages.linkedServerFailedBody, {
				name: server.name,
			})
		}}
		<template #actions>
			<InstancePushButton :instance="instance" primary />
		</template>
	</Admonition>
</template>

<script setup lang="ts">
import { Admonition, useVIntl } from '@modrinth/ui'

import InstancePushButton from '@/components/ui/linked-servers/InstancePushButton.vue'
import type { LinkedServer } from '@/helpers/linked-servers'
import type { GameInstance } from '@/helpers/types'

import { instanceAdmonitionsMessages as messages } from './messages'

/** Shown while the instance's server failed to take it, e.g. the upload never arrived. */
defineProps<{
	instance: GameInstance
	server: LinkedServer
}>()

const { formatMessage } = useVIntl()
</script>
