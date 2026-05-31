<template>
	<div class="flex flex-col gap-6">
		<header>
			<h3 class="m-0 text-lg font-extrabold text-contrast">Network</h3>
			<p class="m-0 mt-1 text-sm text-secondary">The primary port players connect to.</p>
		</header>

		<label class="flex max-w-xs flex-col gap-1">
			<span class="text-sm font-bold text-contrast">Server port</span>
			<StyledInput v-model.number="port" type="number" :min="1024" :max="65535" :step="1" />
			<span class="text-xs text-secondary">Applied on the next restart.</span>
		</label>

		<p class="m-0 text-xs text-secondary">
			Additional ports (voice chat, Bedrock bridge, web maps) and remote tunneling are coming in a
			future update.
		</p>

		<ServerSettingsSaveBar :visible="isDirty" :saving="saving" @save="save" @reset="reset" />
	</div>
</template>

<script setup lang="ts">
import { StyledInput } from '@modrinth/ui'
import { computed, ref } from 'vue'

import ServerSettingsSaveBar from '../ServerSettingsSaveBar.vue'
import { useServerSettings } from '../use-server-settings'

const { core, instanceId, refreshServer, addNotification, handleError } = useServerSettings()

const port = ref(25565)
const savedPort = ref(25565)
const saving = ref(false)

async function load() {
	const instance = await core.getInstance(instanceId.value)
	port.value = instance.port
	savedPort.value = instance.port
}

const isDirty = computed(() => port.value !== savedPort.value)

function reset() {
	port.value = savedPort.value
}

async function save() {
	if (port.value < 1024 || port.value > 65535) {
		addNotification({
			title: 'Invalid port',
			text: 'Port must be between 1024 and 65535.',
			type: 'error',
		})
		return
	}
	saving.value = true
	try {
		await core.patchProperties(instanceId.value, { 'server-port': String(port.value) })
		await refreshServer()
		await load()
		addNotification({ title: 'Saved', text: 'Server port updated.', type: 'success' })
	} catch (error) {
		handleError(error as Error)
	} finally {
		saving.value = false
	}
}

await load()
</script>
