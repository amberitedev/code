<template>
	<NewModal ref="modal" header="Create server" width="520px">
		<form class="flex flex-col gap-4" @submit.prevent="create">
			<label class="flex flex-col gap-2">
				<span class="font-semibold text-contrast">Name</span>
				<StyledInput v-model="name" placeholder="My server" />
			</label>
			<div class="grid grid-cols-1 gap-4 md:grid-cols-2">
				<label class="flex flex-col gap-2">
					<span class="font-semibold text-contrast">Game version</span>
					<StyledInput v-model="gameVersion" placeholder="1.21.4" />
				</label>
				<label class="flex flex-col gap-2">
					<span class="font-semibold text-contrast">Loader</span>
					<select
						v-model="loader"
						class="h-10 rounded-lg border border-solid border-button-border bg-button-bg px-3 text-primary"
					>
						<option value="vanilla">Vanilla</option>
						<option value="paper">Paper</option>
						<option value="fabric">Fabric</option>
						<option value="forge">Forge</option>
						<option value="neoforge">NeoForge</option>
						<option value="quilt">Quilt</option>
					</select>
				</label>
			</div>
			<div class="grid grid-cols-1 gap-4 md:grid-cols-2">
				<label class="flex flex-col gap-2">
					<span class="font-semibold text-contrast">Port</span>
					<StyledInput v-model="port" type="number" min="1" max="65535" />
				</label>
				<label class="flex flex-col gap-2">
					<span class="font-semibold text-contrast">Memory (MB)</span>
					<StyledInput v-model="memory" type="number" min="512" step="256" />
				</label>
			</div>
			<div v-if="error" class="rounded-lg bg-red-highlight p-3 text-sm text-contrast">
				{{ error.message }}
			</div>
			<div class="flex justify-end gap-2">
				<ButtonStyled type="transparent">
					<button type="button" @click="hide">Cancel</button>
				</ButtonStyled>
				<ButtonStyled color="brand">
					<button :disabled="creating || !name.trim() || !gameVersion.trim()" type="submit">
						<PlusIcon />
						{{ creating ? 'Creating...' : 'Create' }}
					</button>
				</ButtonStyled>
			</div>
		</form>
	</NewModal>
</template>

<script setup lang="ts">
import type { CoreModLoader } from '@amberite/amberite-api'
import { PlusIcon } from '@modrinth/assets'
import { ref } from 'vue'

import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import StyledInput from '#ui/components/base/StyledInput.vue'
import NewModal from '#ui/components/modal/NewModal.vue'
import { injectHostingBackend, injectNotificationManager } from '#ui/providers'

const emit = defineEmits<{
	created: [id: string]
}>()

const backend = injectHostingBackend()
const { handleError } = injectNotificationManager()
const modal = ref<InstanceType<typeof NewModal>>()
const name = ref('')
const gameVersion = ref('1.21.4')
const loader = ref<CoreModLoader>('vanilla')
const port = ref('25565')
const memory = ref('2048')
const creating = ref(false)
const error = ref<Error | null>(null)

function show() {
	error.value = null
	modal.value?.show()
}

function hide() {
	modal.value?.hide()
}

async function create() {
	creating.value = true
	error.value = null
	try {
		const server = await backend.createServer({
			name: name.value.trim(),
			game_version: gameVersion.value.trim(),
			loader: loader.value,
			port: Number(port.value),
			memory: {
				min_mb: Number(memory.value),
				max_mb: Number(memory.value),
			},
		})
		hide()
		emit('created', server.id)
	} catch (err) {
		error.value = err as Error
		handleError(err as Error)
	} finally {
		creating.value = false
	}
}

defineExpose({ show, hide })
</script>
