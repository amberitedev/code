<template>
	<NewModal ref="modal" header="Server settings" width="760px" @hide="emit('update:open', false)">
		<div class="flex flex-col gap-5">
			<div class="flex flex-wrap gap-2">
				<ButtonStyled
					v-for="tab in tabs"
					:key="tab.id"
					:type="activeTab === tab.id ? 'standard' : 'transparent'"
				>
					<button @click="activeTab = tab.id">{{ tab.label }}</button>
				</ButtonStyled>
			</div>

			<div v-if="loading" class="rounded-lg bg-surface-3 p-4 text-secondary">Loading settings...</div>
			<div v-else-if="error" class="rounded-lg bg-red-highlight p-4 text-contrast">
				{{ error.message }}
			</div>

			<form v-else-if="activeTab === 'general'" class="flex flex-col gap-4" @submit.prevent="saveGeneral">
				<label class="flex flex-col gap-2">
					<span class="font-semibold text-contrast">Name</span>
					<StyledInput v-model="name" />
				</label>
				<div class="grid grid-cols-1 gap-4 md:grid-cols-2">
					<label class="flex flex-col gap-2">
						<span class="font-semibold text-contrast">Minimum memory (MB)</span>
						<StyledInput v-model="minMemory" type="number" min="0" step="256" />
					</label>
					<label class="flex flex-col gap-2">
						<span class="font-semibold text-contrast">Maximum memory (MB)</span>
						<StyledInput v-model="maxMemory" type="number" min="512" step="256" />
					</label>
				</div>
				<label class="flex flex-col gap-2">
					<span class="font-semibold text-contrast">Java version</span>
					<StyledInput v-model="javaVersion" type="number" min="8" placeholder="Default" />
				</label>
				<div class="flex justify-end">
					<ButtonStyled color="brand">
						<button :disabled="saving" type="submit">Save</button>
					</ButtonStyled>
				</div>
			</form>

			<form v-else-if="activeTab === 'startup'" class="flex flex-col gap-4" @submit.prevent="saveStartup">
				<label class="flex flex-col gap-2">
					<span class="font-semibold text-contrast">JVM arguments</span>
					<StyledInput v-model="jvmArgs" placeholder="Use Core default" />
				</label>
				<label class="flex flex-col gap-2">
					<span class="font-semibold text-contrast">Server arguments</span>
					<StyledInput v-model="serverArgs" placeholder="Use Core default" />
				</label>
				<div class="rounded-lg bg-surface-3 p-3 text-sm text-secondary">
					<div class="font-semibold text-primary">Effective command</div>
					<code class="break-all">{{ startup?.effective_command ?? 'Not available' }}</code>
				</div>
				<div class="flex justify-end">
					<ButtonStyled color="brand">
						<button :disabled="saving" type="submit">Save</button>
					</ButtonStyled>
				</div>
			</form>

			<form v-else class="flex flex-col gap-4" @submit.prevent="saveProperties">
				<label class="flex flex-col gap-2">
					<span class="font-semibold text-contrast">server.properties</span>
					<textarea
						v-model="propertiesText"
						class="min-h-[22rem] resize-y rounded-lg border border-solid border-button-border bg-button-bg p-3 font-mono text-sm text-primary"
						spellcheck="false"
					/>
				</label>
				<div class="flex justify-end">
					<ButtonStyled color="brand">
						<button :disabled="saving" type="submit">Save</button>
					</ButtonStyled>
				</div>
			</form>
		</div>
	</NewModal>
</template>

<script setup lang="ts">
import type { CoreInstance, CoreStartupSettings } from '@amberite/amberite-api'
import { ref, watch } from 'vue'

import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import StyledInput from '#ui/components/base/StyledInput.vue'
import NewModal from '#ui/components/modal/NewModal.vue'
import { injectHostingBackend, injectNotificationManager } from '#ui/providers'

const props = defineProps<{
	open: boolean
	serverId: string
}>()
const emit = defineEmits<{
	'update:open': [value: boolean]
	refresh: []
}>()

const backend = injectHostingBackend()
const { addNotification, handleError } = injectNotificationManager()
const modal = ref<InstanceType<typeof NewModal>>()
const activeTab = ref<'general' | 'startup' | 'properties'>('general')
const loading = ref(false)
const saving = ref(false)
const error = ref<Error | null>(null)
const instance = ref<CoreInstance | null>(null)
const startup = ref<CoreStartupSettings | null>(null)
const properties = ref<Record<string, string>>({})
const name = ref('')
const minMemory = ref('0')
const maxMemory = ref('2048')
const javaVersion = ref('')
const jvmArgs = ref('')
const serverArgs = ref('')
const propertiesText = ref('')
const tabs = [
	{ id: 'general' as const, label: 'General' },
	{ id: 'startup' as const, label: 'Startup' },
	{ id: 'properties' as const, label: 'Properties' },
]

watch(
	() => props.open,
	(open) => {
		if (open) {
			modal.value?.show()
			void load()
		} else {
			modal.value?.hide()
		}
	},
)

async function load() {
	loading.value = true
	error.value = null
	try {
		const [nextInstance, nextStartup, nextProperties] = await Promise.all([
			backend.core.getInstance(props.serverId),
			backend.core.getStartup(props.serverId),
			backend.core.getProperties(props.serverId),
		])
		instance.value = nextInstance
		startup.value = nextStartup
		properties.value = nextProperties
		name.value = nextInstance.name
		minMemory.value = String(nextStartup.memory.min_mb)
		maxMemory.value = String(nextStartup.memory.max_mb)
		javaVersion.value = nextStartup.java_version == null ? '' : String(nextStartup.java_version)
		jvmArgs.value = nextStartup.jvm_args ?? ''
		serverArgs.value = nextStartup.server_args ?? ''
		propertiesText.value = serializeProperties(nextProperties)
	} catch (err) {
		error.value = err as Error
		handleError(err as Error)
	} finally {
		loading.value = false
	}
}

async function saveGeneral() {
	await save(async () => {
		if (instance.value && name.value.trim() !== instance.value.name) {
			await backend.core.renameInstance(props.serverId, name.value.trim())
		}
		await backend.core.updateMemory(props.serverId, Number(minMemory.value), Number(maxMemory.value))
		await backend.core.updateJavaVersion(
			props.serverId,
			javaVersion.value.trim() ? Number(javaVersion.value) : null,
		)
	})
}

async function saveStartup() {
	await save(() =>
		backend.core.updateStartup(props.serverId, {
			jvm_args: jvmArgs.value.trim() || null,
			server_args: serverArgs.value.trim() || null,
		}),
	)
}

async function saveProperties() {
	const next = parseProperties(propertiesText.value)
	const updates: Record<string, string> = {}
	for (const [key, value] of Object.entries(next)) {
		if (properties.value[key] !== value) updates[key] = value
	}
	await save(() => backend.core.patchProperties(props.serverId, updates))
}

async function save(action: () => Promise<unknown>) {
	saving.value = true
	try {
		await action()
		addNotification({
			title: 'Settings saved',
			text: 'Your server settings were updated.',
			type: 'success',
		})
		emit('refresh')
		await load()
	} catch (err) {
		handleError(err as Error)
	} finally {
		saving.value = false
	}
}

function serializeProperties(value: Record<string, string>) {
	return Object.entries(value)
		.sort(([a], [b]) => a.localeCompare(b))
		.map(([key, propertyValue]) => `${key}=${propertyValue}`)
		.join('\n')
}

function parseProperties(value: string) {
	const result: Record<string, string> = {}
	for (const line of value.split(/\r?\n/)) {
		const trimmed = line.trim()
		if (!trimmed || trimmed.startsWith('#')) continue
		const index = trimmed.indexOf('=')
		if (index === -1) continue
		result[trimmed.slice(0, index).trim()] = trimmed.slice(index + 1).trim()
	}
	return result
}
</script>
