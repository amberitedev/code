<script setup lang="ts">
import type { Archon } from '@modrinth/api-client'
import { Button, injectModrinthClient, injectNotificationManager, NewModal } from '@modrinth/ui'
import { ref } from 'vue'

import { ensureCore } from '@/composables/use-cores'

// Decides which Core a new server goes on. Mount it where servers are created and call `pick`.

const client = injectModrinthClient()
const { addNotification } = injectNotificationManager()
const modal = ref<InstanceType<typeof NewModal> | null>(null)
const cores = ref<Archon.Cores.v1.Core[]>([])
let settle: ((core: Archon.Cores.v1.Core | null) => void) | null = null

/**
 * Resolves the Core to create a server on, or null if the user backs out or the account's Cores
 * cannot be loaded. Asks to add a Core first when there is none, and only asks which one when there
 * are several.
 */
async function pick(): Promise<Archon.Cores.v1.Core | null> {
	try {
		if (!(await ensureCore())) return null
		// Servers are only created on the account's own Cores.
		cores.value = (await client.archon.cores_v1.list()).filter((core) => core.owned)
	} catch (cause) {
		console.error('Failed to load Cores', cause)
		addNotification({
			type: 'error',
			title: 'Could not load your Cores',
			text: 'Check your connection, then try again.',
		})
		return null
	}
	if (cores.value.length <= 1) return cores.value[0] ?? null
	return new Promise((resolve) => {
		settle = resolve
		modal.value?.show()
	})
}

function choose(core: Archon.Cores.v1.Core | null) {
	settle?.(core)
	settle = null
	if (core) modal.value?.hide()
}

defineExpose({ pick })
</script>

<template>
	<NewModal
		ref="modal"
		header="Choose a Core"
		max-width="460px"
		width="100%"
		:on-hide="() => choose(null)"
	>
		<div class="flex flex-col gap-2">
			<Button v-for="core in cores" :key="core.id" class="w-full" @click="choose(core)">
				<span class="flex min-w-0 flex-col">
					<span class="truncate">{{ core.name }}</span>
					<span class="truncate text-sm font-normal text-secondary">{{ core.address }}</span>
				</span>
			</Button>
		</div>
	</NewModal>
</template>
