<template>
	<div class="contents">
		<div class="flex flex-row items-center gap-2 rounded-lg">
			<ButtonStyled v-if="isInstalling" type="standard" color="brand" size="large">
				<button disabled class="flex-shrink-0">
					<LoaderCircleIcon class="size-5 animate-spin" /> Installing...
				</button>
			</ButtonStyled>

			<template v-else-if="showRestartButton">
				<ButtonStyled type="standard" color="orange" size="large">
					<button v-tooltip="busyTooltip" :disabled="!canTakeAction" @click="handlePrimaryAction">
						<UpdatedIcon />
						<span>{{ primaryActionText }}</span>
					</button>
				</ButtonStyled>

				<JoinedButtons
					color="red"
					size="large"
					:actions="stopSplitActions"
					:primary-disabled="!canTakeAction"
					:dropdown-disabled="!canKill"
				>
					<template #kill_server>
						<SlashIcon class="h-5 w-5" />
						Kill server
					</template>
				</JoinedButtons>
			</template>

			<template v-else-if="isStopping">
				<JoinedButtons
					color="red"
					size="large"
					:actions="stopSplitActions"
					:primary-disabled="true"
					:dropdown-disabled="!canKill"
					:primary-muted="true"
				>
					<template #kill_server>
						<SlashIcon class="h-5 w-5" />
						Kill server
					</template>
				</JoinedButtons>
			</template>

			<ButtonStyled v-else type="standard" color="brand" size="large">
				<button v-tooltip="busyTooltip" :disabled="!canTakeAction" @click="handlePrimaryAction">
					<PlayIcon />
					<span>{{ primaryActionText }}</span>
				</button>
			</ButtonStyled>
		</div>
	</div>
</template>

<script setup lang="ts">
import {
	LoaderCircleIcon,
	PlayIcon,
	SlashIcon,
	StopCircleIcon,
	UpdatedIcon,
} from '@modrinth/assets'
import { computed } from 'vue'

import { ButtonStyled, type JoinedButtonAction, JoinedButtons } from '#ui/components'
import {
	injectCoreClient,
	injectModrinthServerContext,
	injectNotificationManager,
} from '#ui/providers'

const props = withDefaults(defineProps<{ disabled?: boolean }>(), { disabled: false })

const coreClient = injectCoreClient()
const { addNotification } = injectNotificationManager()
const { serverId, server, powerState, isSyncingContent, busyReasons } =
	injectModrinthServerContext()

const isInstalling = computed(
	() =>
		server.value.status === 'installing' || isSyncingContent.value || busyReasons.value.length > 0,
)
const isRunning = computed(() => powerState.value === 'running')
const isStopping = computed(() => powerState.value === 'stopping')
const isStarting = computed(() => powerState.value === 'starting')
const isTransitioning = computed(() => isStarting.value || isStopping.value)
const showRestartButton = computed(() => isRunning.value || isStarting.value)
const canTakeAction = computed(
	() => !props.disabled && !isTransitioning.value && busyReasons.value.length === 0,
)
const canKill = computed(
	() => !props.disabled && (isRunning.value || isStarting.value || isStopping.value),
)
const busyTooltip = computed(() => {
	if (isStarting.value) return 'Your server is starting'
	return busyReasons.value[0] ? 'Your server is busy' : undefined
})
const primaryActionText = computed(() =>
	showRestartButton.value ? 'Restart' : isStopping.value ? 'Stopping' : 'Start',
)

const stopSplitActions = computed<JoinedButtonAction[]>(() => [
	{
		id: 'stop',
		label: isStopping.value ? 'Stopping' : 'Stop',
		icon: StopCircleIcon,
		action: () => initiateAction('Stop'),
	},
	{
		id: 'kill_server',
		label: 'Kill server',
		icon: SlashIcon,
		action: () => initiateAction('Kill'),
	},
])

async function sendPowerAction(action: 'Start' | 'Stop' | 'Restart' | 'Kill') {
	try {
		if (action === 'Start') await coreClient.start(serverId)
		else if (action === 'Stop') await coreClient.stop(serverId)
		else if (action === 'Restart') await coreClient.restart(serverId)
		else await coreClient.kill(serverId)
	} catch (error) {
		console.error(`[core/server-manage] Failed to ${action.toLowerCase()} server:`, error)
		addNotification({
			type: 'error',
			title: `Failed to ${action.toLowerCase()} server`,
			text: 'An error occurred while performing this action.',
		})
	}
}

function initiateAction(action: 'Start' | 'Stop' | 'Restart' | 'Kill') {
	if (action === 'Kill' ? !canKill.value : !canTakeAction.value) return
	void sendPowerAction(action)
}

function handlePrimaryAction() {
	initiateAction(isRunning.value ? 'Restart' : 'Start')
}
</script>
