<template>
	<div class="flex flex-col gap-4">
		<span class="font-semibold text-contrast">{{ formatMessage(messages.label) }}</span>
		<div class="flex flex-col gap-3">
			<BigOptionButton
				v-for="type in instanceTypeCards"
				:key="type.id"
				:icon="type.icon"
				:title="formatInstanceTypeLabel(type.id)"
				:description="formatMessage(type.description)"
				:selected="
					ctx.instanceTypeClickBehavior.value === 'select' && ctx.instanceType.value === type.id
				"
				@click="selectInstanceType(type.id)"
			/>
		</div>
	</div>
</template>

<script setup lang="ts">
import { MonitorIcon, MonitorSmartphoneIcon, ServerIcon } from '@modrinth/assets'
import { BigOptionButton, defineMessages, useVIntl } from '@modrinth/ui'
import { injectCreationFlowContext } from '@modrinth/ui/src/components/flows/creation-flow-modal/creation-flow-context'
import type { Component } from 'vue'

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
	clientDescription: {
		id: 'app.creation-flow.instance-type.client.description',
		defaultMessage: 'Create a local Minecraft instance for playing on this device.',
	},
	serverDescription: {
		id: 'app.creation-flow.instance-type.server.description',
		defaultMessage: 'Create a Core-managed server instance for hosting.',
	},
	syncedDescription: {
		id: 'app.creation-flow.instance-type.synced.description',
		defaultMessage: 'Create a linked client and server profile that stay in sync.',
	},
})

const instanceTypeCards = [
	{
		id: 'client',
		icon: MonitorIcon,
		description: messages.clientDescription,
	},
	{
		id: 'server',
		icon: ServerIcon,
		description: messages.serverDescription,
	},
	{
		id: 'synced',
		icon: MonitorSmartphoneIcon,
		description: messages.syncedDescription,
	},
] satisfies { id: InstanceType; icon: Component; description: (typeof messages)['clientDescription'] }[]

function formatInstanceTypeLabel(type: InstanceType) {
	return formatMessage(messages[type])
}

function selectInstanceType(type: InstanceType) {
	ctx.instanceType.value = type
	if (ctx.instanceTypeClickBehavior.value === 'continue') {
		ctx.modal.value?.setStage('custom-setup')
	}
}
</script>
