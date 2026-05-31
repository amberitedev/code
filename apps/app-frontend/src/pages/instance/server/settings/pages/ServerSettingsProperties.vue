<template>
	<div class="flex flex-col gap-6">
		<header>
			<h3 class="m-0 text-lg font-extrabold text-contrast">Properties</h3>
			<p class="m-0 mt-1 text-sm text-secondary">
				Edit <code>server.properties</code>. Changes apply on the next restart.
			</p>
		</header>

		<StyledInput
			v-model="search"
			type="text"
			:icon="SearchIcon"
			placeholder="Search properties"
			clearable
		/>

		<div
			class="flex flex-col divide-y divide-divider rounded-xl border border-solid border-divider"
		>
			<div
				v-for="key in filteredKeys"
				:key="key"
				class="flex items-center justify-between gap-4 px-4 py-3"
			>
				<label :for="`prop-${key}`" class="min-w-0 break-words font-mono text-sm text-contrast">
					{{ key }}
				</label>
				<Toggle
					v-if="isBoolean(form[key])"
					:id="`prop-${key}`"
					:model-value="form[key] === 'true'"
					@update:model-value="(v: boolean) => (form[key] = v ? 'true' : 'false')"
				/>
				<StyledInput v-else :id="`prop-${key}`" v-model="form[key]" type="text" class="max-w-xs" />
			</div>
			<p v-if="filteredKeys.length === 0" class="px-4 py-6 text-center text-sm text-secondary">
				No properties match your search.
			</p>
		</div>

		<ServerSettingsSaveBar :visible="isDirty" :saving="saving" @save="save" @reset="reset" />
	</div>
</template>

<script setup lang="ts">
import { SearchIcon } from '@modrinth/assets'
import { StyledInput, Toggle } from '@modrinth/ui'
import { computed, ref } from 'vue'

import ServerSettingsSaveBar from '../ServerSettingsSaveBar.vue'
import { useServerSettings } from '../use-server-settings'

const { core, instanceId, addNotification, handleError } = useServerSettings()

const search = ref('')
const form = ref<Record<string, string>>({})
let saved: Record<string, string> = {}
const saving = ref(false)

function isBoolean(value: string) {
	return value === 'true' || value === 'false'
}

async function load() {
	const properties = await core.getProperties(instanceId.value)
	form.value = { ...properties }
	saved = { ...properties }
}

const sortedKeys = computed(() => Object.keys(form.value).sort())

const filteredKeys = computed(() => {
	const query = search.value.trim().toLowerCase()
	if (!query) return sortedKeys.value
	return sortedKeys.value.filter((key) => key.toLowerCase().includes(query))
})

const isDirty = computed(() => sortedKeys.value.some((key) => form.value[key] !== saved[key]))

function reset() {
	form.value = { ...saved }
}

async function save() {
	const updates: Record<string, string> = {}
	for (const key of sortedKeys.value) {
		if (form.value[key] !== saved[key]) updates[key] = form.value[key]
	}
	if (Object.keys(updates).length === 0) return
	saving.value = true
	try {
		await core.patchProperties(instanceId.value, updates)
		await load()
		addNotification({ title: 'Saved', text: 'Server properties updated.', type: 'success' })
	} catch (error) {
		handleError(error as Error)
	} finally {
		saving.value = false
	}
}

await load()
</script>
