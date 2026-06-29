import { computed, type Ref } from 'vue'

import { useVIntl } from '#ui/composables/i18n'
import { useServerPermissions } from '#ui/composables/server-permissions'
import {
	injectHostingBackend,
	injectModrinthClient,
	injectModrinthServerContext,
	injectNotificationManager,
} from '#ui/providers'

export type PowerAction = 'Start' | 'Stop' | 'Restart' | 'Kill'

export function useServerPowerAction(options?: { disabled?: Ref<boolean> }) {
	const { formatMessage } = useVIntl()
	const client = injectModrinthClient()
	const hostingBackend = injectHostingBackend(null)
	const { serverId, server, powerState, isSyncingContent, busyReasons } =
		injectModrinthServerContext()
	const { addNotification } = injectNotificationManager()
	const { canUsePowerActions, permissionDeniedMessage } = useServerPermissions()

	const isInstalling = computed(
		() =>
			server.value.status === 'installing' ||
			isSyncingContent.value ||
			busyReasons.value.some(
				(r) =>
					r.reason.id === 'servers.busy.installing' ||
					r.reason.id === 'servers.busy.syncing-content',
			),
	)
	const isRunning = computed(() => powerState.value === 'running')
	const isStopping = computed(() => powerState.value === 'stopping')
	const isStarting = computed(() => powerState.value === 'starting')
	const isTransitioning = computed(() => isStarting.value || isStopping.value)

	const showStopSplit = computed(() => isRunning.value || isStarting.value || isStopping.value)
	const showRestartButton = computed(() => isRunning.value || isStarting.value)

	const isBlockedByPropsBusyOrPermission = computed(
		() =>
			!canUsePowerActions.value ||
			Boolean(options?.disabled?.value) ||
			busyReasons.value.length > 0,
	)

	const busyTooltip = computed(() => {
		if (!canUsePowerActions.value) return permissionDeniedMessage.value
		if (isStarting.value) return 'Your server is starting'
		return busyReasons.value.length > 0 ? formatMessage(busyReasons.value[0].reason) : undefined
	})

	const canTakeAction = computed(
		() => !isTransitioning.value && !isBlockedByPropsBusyOrPermission.value,
	)

	const canKill = computed(
		() =>
			!isBlockedByPropsBusyOrPermission.value &&
			(isStopping.value || isRunning.value || isStarting.value),
	)

	const primaryActionText = computed(() => {
		switch (powerState.value) {
			case 'running':
			case 'starting':
				return 'Restart'
			case 'stopping':
				return 'Stopping'
			default:
				return 'Start'
		}
	})

	async function refreshCorePowerState() {
		if (!hostingBackend) return
		const instance = await hostingBackend.getServer(serverId)
		powerState.value = instance.status === 'offline' ? 'stopped' : instance.status
	}

	async function sendPowerAction(action: PowerAction) {
		const previousPowerState = powerState.value
		let usedHostingBackend = false

		try {
			if (hostingBackend) {
				usedHostingBackend = true
				switch (action) {
					case 'Start':
						powerState.value = 'starting'
						await hostingBackend.core.start(serverId)
						await refreshCorePowerState()
						return
					case 'Stop':
						powerState.value = 'stopping'
						await hostingBackend.core.stop(serverId)
						await refreshCorePowerState()
						return
					case 'Restart':
						powerState.value = 'starting'
						await hostingBackend.core.restart(serverId)
						await refreshCorePowerState()
						return
					case 'Kill':
						powerState.value = 'stopping'
						await hostingBackend.core.kill(serverId)
						await refreshCorePowerState()
						return
				}
			}
			await client.archon.servers_v0.power(serverId, action)
		} catch (error) {
			if (usedHostingBackend) {
				powerState.value = previousPowerState
			}
			console.error(`Error performing ${action} on server:`, error)
			addNotification({
				type: 'error',
				title: `Failed to ${action.toLowerCase()} server`,
				text: 'An error occurred while performing this action.',
			})
		}
	}

	function initiateAction(action: PowerAction) {
		if (action === 'Kill') {
			if (!canKill.value) return
		} else {
			if (!canTakeAction.value) return
		}
		void sendPowerAction(action)
	}

	function handlePrimaryAction() {
		initiateAction(isRunning.value ? 'Restart' : 'Start')
	}

	return {
		isInstalling,
		isRunning,
		isStopping,
		isTransitioning,
		showStopSplit,
		showRestartButton,
		busyTooltip,
		canTakeAction,
		canKill,
		primaryActionText,
		sendPowerAction,
		initiateAction,
		handlePrimaryAction,
	}
}
