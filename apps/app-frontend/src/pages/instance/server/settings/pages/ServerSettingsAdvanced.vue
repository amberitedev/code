<template>
	<div class="flex flex-col gap-6">
		<header>
			<h3 class="m-0 text-lg font-extrabold text-contrast">Advanced</h3>
			<p class="m-0 mt-1 text-sm text-secondary">
				Extra JVM and server arguments for the launch command.
			</p>
		</header>

		<label class="flex flex-col gap-1">
			<span class="text-sm font-bold text-contrast">JVM arguments</span>
			<span class="text-xs text-secondary">Inserted after the memory flags.</span>
			<StyledInput v-model="form.jvmArgs" type="text" placeholder="-XX:+UseG1GC" />
		</label>

		<label class="flex flex-col gap-1">
			<span class="text-sm font-bold text-contrast">Server arguments</span>
			<span class="text-xs text-secondary">Appended to the end of the command.</span>
			<StyledInput v-model="form.serverArgs" type="text" placeholder="--nogui" />
		</label>

		<div class="flex flex-col gap-2">
			<span class="text-sm font-bold text-contrast">Effective launch command</span>
			<pre
				class="m-0 overflow-x-auto whitespace-pre-wrap break-all rounded-xl border border-solid border-divider bg-bg p-3 font-mono text-xs text-secondary"
				>{{ preview }}</pre
			>
		</div>

		<ServerSettingsSaveBar :visible="isDirty" :saving="saving" @save="save" @reset="reset" />
	</div>
</template>

<script setup lang="ts">
import { StyledInput } from '@modrinth/ui'
import { computed, reactive, ref } from 'vue'

import ServerSettingsSaveBar from '../ServerSettingsSaveBar.vue'
import { useServerSettings } from '../use-server-settings'

const { core, instanceId, addNotification, handleError } = useServerSettings()

type AdvancedForm = { jvmArgs: string; serverArgs: string }

const form = reactive<AdvancedForm>({ jvmArgs: '', serverArgs: '' })
let saved: AdvancedForm = { ...form }
const effectiveCommand = ref('')
const saving = ref(false)

async function load() {
	const startup = await core.getStartup(instanceId.value)
	Object.assign(form, {
		jvmArgs: startup.jvm_args ?? '',
		serverArgs: startup.server_args ?? '',
	})
	saved = { ...form }
	effectiveCommand.value = startup.effective_command
}

const isDirty = computed(
	() => form.jvmArgs !== saved.jvmArgs || form.serverArgs !== saved.serverArgs,
)

const preview = computed(() =>
	isDirty.value ? 'Save to refresh preview…' : effectiveCommand.value,
)

function reset() {
	Object.assign(form, saved)
}

async function save() {
	saving.value = true
	try {
		await core.updateStartup(instanceId.value, {
			jvm_args: form.jvmArgs.trim() || null,
			server_args: form.serverArgs.trim() || null,
		})
		await load()
		addNotification({ title: 'Saved', text: 'Startup arguments updated.', type: 'success' })
	} catch (error) {
		handleError(error as Error)
	} finally {
		saving.value = false
	}
}

await load()
</script>
