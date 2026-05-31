<template>
	<div class="flex flex-col gap-6">
		<header>
			<h3 class="m-0 text-lg font-extrabold text-contrast">General</h3>
			<p class="m-0 mt-1 text-sm text-secondary">Name, memory, and Java runtime for this server.</p>
		</header>

		<label class="flex flex-col gap-1">
			<span class="text-sm font-bold text-contrast">Server name</span>
			<StyledInput v-model="form.name" type="text" :maxlength="64" placeholder="My server" />
		</label>

		<div class="flex flex-col gap-1">
			<span class="text-sm font-bold text-contrast">Memory (MB)</span>
			<p class="m-0 text-xs text-secondary">Applied the next time the server starts.</p>
			<div class="mt-1 flex items-center gap-3">
				<label class="flex flex-1 flex-col gap-1">
					<span class="text-xs text-secondary">Minimum</span>
					<StyledInput v-model.number="form.minMb" type="number" :min="256" :step="256" />
				</label>
				<label class="flex flex-1 flex-col gap-1">
					<span class="text-xs text-secondary">Maximum</span>
					<StyledInput v-model.number="form.maxMb" type="number" :min="512" :step="256" />
				</label>
			</div>
		</div>

		<label class="flex max-w-xs flex-col gap-1">
			<span class="text-sm font-bold text-contrast">Java version</span>
			<Combobox v-model="form.javaVersion" name="Java version" :options="javaOptions" />
		</label>

		<ServerSettingsSaveBar :visible="isDirty" :saving="saving" @save="save" @reset="reset" />
	</div>
</template>

<script setup lang="ts">
import { Combobox, StyledInput } from '@modrinth/ui'
import { computed, reactive, ref } from 'vue'

import ServerSettingsSaveBar from '../ServerSettingsSaveBar.vue'
import { useServerSettings } from '../use-server-settings'

const { core, instanceId, refreshServer, addNotification, handleError } = useServerSettings()

const javaOptions = [
	{ value: null, label: 'Automatic' },
	{ value: 8, label: 'Java 8' },
	{ value: 11, label: 'Java 11' },
	{ value: 17, label: 'Java 17' },
	{ value: 21, label: 'Java 21' },
]

type GeneralForm = { name: string; minMb: number; maxMb: number; javaVersion: number | null }

const form = reactive<GeneralForm>({ name: '', minMb: 1024, maxMb: 2048, javaVersion: null })
let saved: GeneralForm = { ...form }
const saving = ref(false)

async function load() {
	const instance = await core.getInstance(instanceId.value)
	Object.assign(form, {
		name: instance.name,
		minMb: instance.memory.min_mb,
		maxMb: instance.memory.max_mb,
		javaVersion: instance.java_version,
	})
	saved = { ...form }
}

const isDirty = computed(
	() =>
		form.name !== saved.name ||
		form.minMb !== saved.minMb ||
		form.maxMb !== saved.maxMb ||
		form.javaVersion !== saved.javaVersion,
)

function reset() {
	Object.assign(form, saved)
}

async function save() {
	if (form.maxMb < form.minMb) {
		addNotification({
			title: 'Invalid memory',
			text: 'Maximum memory must be at least the minimum.',
			type: 'error',
		})
		return
	}
	saving.value = true
	try {
		if (form.name !== saved.name) await core.renameInstance(instanceId.value, form.name.trim())
		if (form.minMb !== saved.minMb || form.maxMb !== saved.maxMb) {
			await core.updateMemory(instanceId.value, form.minMb, form.maxMb)
		}
		if (form.javaVersion !== saved.javaVersion) {
			await core.updateJavaVersion(instanceId.value, form.javaVersion)
		}
		await refreshServer()
		await load()
		addNotification({ title: 'Saved', text: 'General settings updated.', type: 'success' })
	} catch (error) {
		handleError(error as Error)
	} finally {
		saving.value = false
	}
}

await load()
</script>
