<script setup lang="ts">
import type { CoreMetadata } from '@amberite/amberite-api'
import { ButtonStyled } from '@modrinth/ui'
import { onMounted, reactive, ref } from 'vue'

import { useCoreCall } from '@/composables/useCoreCall'

defineOptions({ name: 'CoreSettingsPage' })

const form = reactive({
	name: '',
	description: '',
	subdomain: '',
	run_mode: 'manual' as CoreMetadata['run_mode'],
})
const setupMode = ref<CoreMetadata['setup_mode'] | null>(null)
const saved = ref(false)

const { data: metadata, loading, error, execute: load } = useCoreCall((c) => c.getCoreMetadata())

const {
	loading: saving,
	error: saveError,
	execute: save,
} = useCoreCall((c) =>
	c.updateCoreMetadata({
		name: form.name,
		description: form.description || null,
		subdomain: form.subdomain || null,
		run_mode: form.run_mode,
	}),
)

function hydrate(m: CoreMetadata | null) {
	if (!m) return
	form.name = m.name
	form.description = m.description ?? ''
	form.subdomain = m.subdomain ?? ''
	form.run_mode = m.run_mode
	setupMode.value = m.setup_mode
}

async function onSave() {
	saved.value = false
	const result = await save()
	if (result) {
		hydrate(result)
		saved.value = true
	}
}

onMounted(async () => {
	await load()
	hydrate(metadata.value)
})
</script>

<template>
	<div class="flex flex-col gap-4 w-full">
		<div v-if="loading" class="text-secondary">Loading Core settings…</div>
		<p v-else-if="error" class="text-red m-0">Couldn't reach this Core: {{ error.message }}</p>

		<template v-else>
			<label class="flex flex-col gap-1">
				<span class="font-semibold">Core name</span>
				<input v-model="form.name" class="rounded-lg bg-bg-input px-3 py-2" />
			</label>

			<label class="flex flex-col gap-1">
				<span class="font-semibold">Description</span>
				<textarea v-model="form.description" rows="3" class="rounded-lg bg-bg-input px-3 py-2" />
			</label>

			<label class="flex flex-col gap-1">
				<span class="font-semibold">Subdomain</span>
				<input
					v-model="form.subdomain"
					class="rounded-lg bg-bg-input px-3 py-2"
					placeholder="my-core"
				/>
			</label>

			<label class="flex flex-col gap-1">
				<span class="font-semibold">Run mode</span>
				<select v-model="form.run_mode" class="rounded-lg bg-bg-input px-3 py-2">
					<option value="manual">Manual</option>
					<option value="app_open">When app opens</option>
					<option value="startup">On system startup</option>
				</select>
			</label>

			<p v-if="setupMode" class="text-secondary text-sm m-0">
				Setup mode: <strong>{{ setupMode }}</strong> (set during pairing)
			</p>

			<div class="flex items-center gap-3">
				<ButtonStyled color="brand">
					<button :disabled="saving" @click="onSave">
						{{ saving ? 'Saving…' : 'Save changes' }}
					</button>
				</ButtonStyled>
				<span v-if="saved" class="text-green text-sm">Saved.</span>
				<span v-if="saveError" class="text-red text-sm">{{ saveError.message }}</span>
			</div>
		</template>
	</div>
</template>
