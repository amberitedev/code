<template>
	<NewModal ref="modal" :header="formatMessage(messages.header)" width="560px">
		<form class="flex flex-col gap-6" @submit.prevent="create">
			<div class="flex flex-col gap-1">
				<p class="m-0 text-sm text-secondary">
					{{ formatMessage(messages.description) }}
				</p>
			</div>

			<div class="flex flex-col gap-4">
				<label class="flex flex-col gap-2">
					<span class="text-sm font-semibold text-contrast">
						{{ formatMessage(messages.nameLabel) }}
					</span>
					<StyledInput v-model="name" :placeholder="formatMessage(messages.namePlaceholder)" />
				</label>

				<div class="grid grid-cols-1 gap-4 md:grid-cols-2">
					<label class="flex flex-col gap-2">
						<span class="text-sm font-semibold text-contrast">
							{{ formatMessage(messages.versionLabel) }}
						</span>
						<StyledInput
							v-model="gameVersion"
							:placeholder="formatMessage(messages.versionPlaceholder)"
						/>
					</label>

					<label class="flex flex-col gap-2">
						<span class="text-sm font-semibold text-contrast">
							{{ formatMessage(messages.loaderLabel) }}
						</span>
						<select
							v-model="loader"
							class="h-9 rounded-xl border border-solid border-surface-5 bg-surface-4 px-3 text-base font-medium text-primary outline-none transition-[box-shadow,color] focus:ring-4 focus:ring-brand-shadow focus:text-contrast"
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
						<span class="text-sm font-semibold text-contrast">
							{{ formatMessage(messages.portLabel) }}
						</span>
						<StyledInput v-model="port" type="number" min="1" max="65535" />
					</label>

					<label class="flex flex-col gap-2">
						<span class="text-sm font-semibold text-contrast">
							{{ formatMessage(messages.memoryLabel) }}
						</span>
						<StyledInput v-model="memory" type="number" min="512" step="256" />
					</label>
				</div>
			</div>

			<div class="rounded-2xl border border-solid border-surface-5 bg-surface-2 p-4">
				<p class="m-0 text-sm text-secondary">
					{{ formatMessage(messages.detailsNote) }}
				</p>
			</div>

			<div v-if="error" class="rounded-lg bg-red-highlight p-3 text-sm text-contrast">
				{{ error.message }}
			</div>

			<div class="flex justify-end gap-2">
				<ButtonStyled type="transparent">
					<button type="button" @click="hide">
						{{ formatMessage(messages.cancelButton) }}
					</button>
				</ButtonStyled>
				<ButtonStyled color="brand">
					<button
						:disabled="creating || !name.trim() || !gameVersion.trim()"
						type="submit"
					>
						<PlusIcon />
						{{
							creating
								? formatMessage(messages.creatingButton)
								: formatMessage(messages.createButton)
						}}
					</button>
				</ButtonStyled>
			</div>
		</form>
	</NewModal>
</template>

<script setup lang="ts">
import type { CoreModLoader } from '@amberite/amberite-api'
import { PlusIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { ref } from 'vue'

import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import StyledInput from '#ui/components/base/StyledInput.vue'
import NewModal from '#ui/components/modal/NewModal.vue'
import { injectHostingBackend, injectNotificationManager } from '#ui/providers'

const emit = defineEmits<{
	created: [path: string]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
	header: { id: 'servers.manage.create.header', defaultMessage: 'New server' },
	description: {
		id: 'servers.manage.create.description',
		defaultMessage: 'Create a new server and choose the Minecraft version, loader, and resources.',
	},
	nameLabel: { id: 'servers.manage.create.name-label', defaultMessage: 'Name' },
	namePlaceholder: {
		id: 'servers.manage.create.name-placeholder',
		defaultMessage: 'My server',
	},
	versionLabel: { id: 'servers.manage.create.version-label', defaultMessage: 'Game version' },
	versionPlaceholder: {
		id: 'servers.manage.create.version-placeholder',
		defaultMessage: '1.21.4',
	},
	loaderLabel: { id: 'servers.manage.create.loader-label', defaultMessage: 'Loader' },
	portLabel: { id: 'servers.manage.create.port-label', defaultMessage: 'Port' },
	memoryLabel: { id: 'servers.manage.create.memory-label', defaultMessage: 'Memory (MB)' },
	detailsNote: {
		id: 'servers.manage.create.details-note',
		defaultMessage: 'Port and memory are used immediately when the server is created.',
	},
	cancelButton: { id: 'servers.manage.create.cancel-button', defaultMessage: 'Cancel' },
	createButton: { id: 'servers.manage.create.create-button', defaultMessage: 'Create server' },
	creatingButton: {
		id: 'servers.manage.create.creating-button',
		defaultMessage: 'Creating...',
	},
})

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

function resetForm() {
	name.value = ''
	gameVersion.value = '1.21.4'
	loader.value = 'vanilla'
	port.value = '25565'
	memory.value = '2048'
	error.value = null
}

function show() {
	resetForm()
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
		emit('created', server.path)
	} catch (err) {
		error.value = err as Error
		handleError(err as Error)
	} finally {
		creating.value = false
	}
}

defineExpose({ show, hide })
</script>
