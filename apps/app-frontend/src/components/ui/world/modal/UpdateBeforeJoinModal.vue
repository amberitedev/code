<script setup lang="ts">
import { DownloadIcon, PlayIcon } from '@modrinth/assets'
import { Button, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { ref } from 'vue'

import { install_update_shared_instance, wait_for_install_job } from '@/helpers/install'
import { useSharedInstanceErrors } from '@/helpers/shared-instance-errors'
import { injectAppEvents } from '@/providers/app-events'

/**
 * Asked when joining a hosted server from a shared instance that is behind its latest version.
 * `join` runs once the user has chosen, after the update when they asked for one.
 */
const { formatMessage } = useVIntl()
const appEvents = injectAppEvents()
const { notifySharedInstanceError } = useSharedInstanceErrors()

const modal = ref<InstanceType<typeof NewModal> | null>(null)
const updating = ref(false)
const instanceId = ref('')
const hostName = ref('')
let join: () => void | Promise<void> = () => {}

function show(instance: string, host: string, onJoin: typeof join) {
	instanceId.value = instance
	hostName.value = host
	join = onJoin
	modal.value?.show()
}

async function updateAndStart() {
	updating.value = true
	try {
		const job = await install_update_shared_instance(instanceId.value)
		await wait_for_install_job(appEvents, job.job_id)
	} catch (error) {
		notifySharedInstanceError(error)
		return
	} finally {
		updating.value = false
	}
	modal.value?.hide()
	await join()
}

async function startWithoutUpdating() {
	modal.value?.hide()
	await join()
}

const messages = defineMessages({
	title: {
		id: 'instance.worlds.update-before-join.title',
		defaultMessage: 'This instance has an update',
	},
	description: {
		id: 'instance.worlds.update-before-join.description',
		defaultMessage:
			"{host}'s server may already be on the new version. Update to be sure you can join.",
	},
	updateAndStart: {
		id: 'instance.worlds.update-before-join.update-and-start',
		defaultMessage: 'Update and start',
	},
	updating: {
		id: 'instance.worlds.update-before-join.updating',
		defaultMessage: 'Updating...',
	},
	startWithoutUpdating: {
		id: 'instance.worlds.update-before-join.start-without-updating',
		defaultMessage: 'Start without updating',
	},
})

defineExpose({ show })
</script>

<template>
	<NewModal ref="modal" :header="formatMessage(messages.title)" max-width="540px">
		<p class="m-0 text-secondary">
			{{ formatMessage(messages.description, { host: hostName }) }}
		</p>
		<template #actions>
			<div class="flex justify-end gap-2">
				<Button
					type="outlined"
					class="whitespace-nowrap"
					:disabled="updating"
					@click="startWithoutUpdating"
				>
					<PlayIcon />
					{{ formatMessage(messages.startWithoutUpdating) }}
				</Button>
				<Button
					type="colored"
					color="brand"
					class="whitespace-nowrap"
					:disabled="updating"
					@click="updateAndStart"
				>
					<DownloadIcon />
					{{ formatMessage(updating ? messages.updating : messages.updateAndStart) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>
