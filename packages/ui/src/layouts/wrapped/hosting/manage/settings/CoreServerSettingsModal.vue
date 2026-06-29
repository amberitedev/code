<template>
	<NewModal
		ref="modal"
		:max-width="'min(980px, calc(95vw - 2rem))'"
		:width="'min(980px, calc(95vw - 2rem))'"
		no-padding
		:on-hide="handleHide"
	>
		<template #title>
			<span class="flex items-center gap-2 text-lg font-semibold text-primary">
				{{ instance?.name || 'Server' }} <ChevronRightIcon />
				<span class="font-extrabold text-contrast">Settings</span>
			</span>
		</template>

		<div class="grid grid-cols-[220px_minmax(0,1fr)] p-6 pb-3 pr-0">
			<div
				class="flex min-w-[200px] flex-col gap-1 border-0 border-r-[1px] border-solid border-divider pr-4"
			>
				<button
					v-for="tab in tabs"
					:key="tab.id"
					:class="[
						'flex items-center gap-2 rounded-xl border border-solid px-4 py-2 text-left text-sm font-semibold transition-all',
						activeTab === tab.id
							? 'border-transparent bg-button-bgSelected text-button-textSelected'
							: 'border-transparent bg-transparent text-button-text hover:bg-button-bg hover:text-contrast',
					]"
					type="button"
					@click="activeTab = tab.id"
				>
					<component :is="tab.icon" class="h-4 w-4 shrink-0" />
					<span>{{ tab.label }}</span>
				</button>
			</div>

			<div class="min-w-0">
				<div class="max-h-[min(65vh,600px)] overflow-y-auto px-6 pb-6">
					<StageContentTransition
						:content-key="activeTab"
						:active-index="activeTabIndex"
						axis="vertical"
					>
						<div v-if="loading" class="rounded-2xl bg-surface-2 p-4 text-secondary">
							Loading settings...
						</div>
						<div v-else-if="error" class="rounded-lg bg-red-highlight p-4 text-contrast">
							{{ error.message }}
						</div>

						<form
							v-else-if="activeTab === 'general'"
							class="flex flex-col gap-6"
							@submit.prevent="saveGeneral"
						>
							<div class="flex flex-col gap-2.5">
								<label for="core-server-name" class="text-lg font-semibold text-contrast">
									Name
								</label>
								<StyledInput id="core-server-name" v-model="name" wrapper-class="w-full" />
							</div>

							<div class="rounded-2xl border border-solid border-surface-5 p-4">
								<div class="grid grid-cols-1 gap-4 md:grid-cols-2">
									<label class="flex flex-col gap-2">
										<span class="font-semibold text-contrast">Minimum memory (MB)</span>
										<StyledInput
											v-model="minMemory"
											type="number"
											min="0"
											step="256"
											wrapper-class="w-full"
										/>
									</label>
									<label class="flex flex-col gap-2">
										<span class="font-semibold text-contrast">Maximum memory (MB)</span>
										<StyledInput
											v-model="maxMemory"
											type="number"
											min="512"
											step="256"
											wrapper-class="w-full"
										/>
									</label>
									<label class="flex flex-col gap-2 md:col-span-2">
										<span class="font-semibold text-contrast">Java version</span>
										<StyledInput
											v-model="javaVersion"
											type="number"
											min="8"
											placeholder="Default"
											wrapper-class="w-full md:max-w-[320px]"
										/>
									</label>
								</div>
							</div>

							<div class="flex justify-end">
								<ButtonStyled color="brand">
									<button :disabled="saving" type="submit">Save</button>
								</ButtonStyled>
							</div>
						</form>

						<form
							v-else-if="activeTab === 'startup'"
							class="flex flex-col gap-6"
							@submit.prevent="saveStartup"
						>
							<div class="flex flex-col gap-2.5">
								<label for="core-jvm-args" class="text-lg font-semibold text-contrast">
									JVM arguments
								</label>
								<StyledInput
									id="core-jvm-args"
									v-model="jvmArgs"
									placeholder="Use default"
									wrapper-class="w-full"
								/>
							</div>

							<div class="flex flex-col gap-2.5">
								<label for="core-server-args" class="text-lg font-semibold text-contrast">
									Server arguments
								</label>
								<StyledInput
									id="core-server-args"
									v-model="serverArgs"
									placeholder="Use default"
									wrapper-class="w-full"
								/>
							</div>

							<div
								class="rounded-2xl border border-solid border-surface-5 bg-surface-2 p-4 text-sm text-secondary"
							>
								<div class="mb-2 text-base font-semibold text-contrast">Effective command</div>
								<code class="break-all text-primary">{{
									startup?.effective_command ?? 'Not available'
								}}</code>
							</div>

							<div class="flex justify-end">
								<ButtonStyled color="brand">
									<button :disabled="saving" type="submit">Save</button>
								</ButtonStyled>
							</div>
						</form>

						<form v-else class="flex flex-col gap-6" @submit.prevent="saveProperties">
							<div class="flex flex-col gap-2.5">
								<label for="core-server-properties" class="text-lg font-semibold text-contrast">
									server.properties
								</label>
								<p class="m-0 text-sm text-secondary">
									Edit the generated server properties directly for this server.
								</p>
								<textarea
									id="core-server-properties"
									v-model="propertiesText"
									class="min-h-[22rem] resize-y rounded-2xl border border-solid border-surface-5 bg-button-bg p-3 font-mono text-sm text-primary"
									spellcheck="false"
								/>
							</div>

							<div class="flex justify-end">
								<ButtonStyled color="brand">
									<button :disabled="saving" type="submit">Save</button>
								</ButtonStyled>
							</div>
						</form>
					</StageContentTransition>
				</div>
			</div>
		</div>
	</NewModal>
</template>

<script setup lang="ts">
import type { CoreInstance, CoreStartupSettings } from '@amberite/amberite-api'
import { ChevronRightIcon, FileIcon, SettingsIcon, TerminalSquareIcon } from '@modrinth/assets'
import { computed, ref, watch } from 'vue'

import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import StageContentTransition from '#ui/components/base/StageContentTransition.vue'
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

type TabId = 'general' | 'startup' | 'properties'

const backend = injectHostingBackend()
const { addNotification, handleError } = injectNotificationManager()
const modal = ref<InstanceType<typeof NewModal>>()
const activeTab = ref<TabId>('general')
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
	{ id: 'general' as const, label: 'General', icon: SettingsIcon },
	{ id: 'startup' as const, label: 'Startup', icon: TerminalSquareIcon },
	{ id: 'properties' as const, label: 'Properties', icon: FileIcon },
]
const activeTabIndex = computed(() => tabs.findIndex((tab) => tab.id === activeTab.value))

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

function handleHide() {
	emit('update:open', false)
}

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
		await backend.core.updateMemory(
			props.serverId,
			Number(minMemory.value),
			Number(maxMemory.value),
		)
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
