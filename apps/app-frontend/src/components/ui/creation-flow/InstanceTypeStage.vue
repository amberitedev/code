<template>
	<div class="flex flex-col gap-2">
		<span class="font-semibold text-contrast">{{ formatMessage(messages.label) }}</span>
		<Chips
			v-model="ctx.instanceType.value"
			:items="instanceTypes"
			:format-label="formatInstanceTypeLabel"
		/>
	</div>
</template>

<script setup lang="ts">
import { Chips, defineMessages, useVIntl } from '@modrinth/ui'
import { injectCreationFlowContext } from '@modrinth/ui/src/components/flows/creation-flow-modal/creation-flow-context'

import type { InstanceCreationFlowContextValue, InstanceType } from './types'

const ctx = injectCreationFlowContext() as InstanceCreationFlowContextValue
const { formatMessage } = useVIntl()

const messages = defineMessages({
	label: {
		id: 'app.creation-flow.instance-type.label',
		defaultMessage: 'Instance type',
	},
	client: {
		id: 'app.creation-flow.instance-type.client',
		defaultMessage: 'Client',
	},
	server: {
		id: 'app.creation-flow.instance-type.server',
		defaultMessage: 'Server',
	},
	synced: {
		id: 'app.creation-flow.instance-type.synced',
		defaultMessage: 'Synced',
	},
})

const instanceTypes: InstanceType[] = ['client', 'server', 'synced']

function formatInstanceTypeLabel(type: InstanceType) {
	return formatMessage(messages[type])
}
</script>
