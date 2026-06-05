<script setup lang="ts">
import type { CoreMetadata } from '@amberite/amberite-api'
import { SaveIcon, ServerStackIcon, TrashIcon, TriangleAlertIcon } from '@modrinth/assets'
import { ButtonStyled, NewModal, StyledInput, Toggle } from '@modrinth/ui'
import { computed, onMounted, reactive, ref, useTemplateRef, watch } from 'vue'

import { useCorePreview } from '@/components/core/use-core-preview'
import { useCoreCall } from '@/composables/useCoreCall'

defineOptions({ name: 'CoreSettingsPage' })

const dangerModal = useTemplateRef<InstanceType<typeof NewModal>>('dangerModal')
const form = reactive({
	name: '',
	description: '',
	banner: '',
	subdomain: '',
	run_mode: 'manual' as CoreMetadata['run_mode'],
})
const setupMode = ref<CoreMetadata['setup_mode'] | null>(null)
const saved = ref(false)
const { isPreviewConnected, isPreviewLocal } = useCorePreview()
const startOnStartup = computed({
	get: () => form.run_mode === 'startup',
	set: (value) => (form.run_mode = value ? 'startup' : 'manual'),
})
const runInBackground = computed({
	get: () => form.run_mode === 'startup' || form.run_mode === 'app_open',
	set: (value) => (form.run_mode = value ? 'app_open' : 'manual'),
})
const isLocal = computed(() => (isPreviewConnected.value ? isPreviewLocal.value : setupMode.value === 'local'))
const { data: metadata, loading, error, execute: load } = useCoreCall((c) => c.getCoreMetadata())
const { loading: saving, error: saveError, execute: save } = useCoreCall((c) =>
	c.updateCoreMetadata({
		name: form.name,
		description: form.description || null,
		banner: form.banner || null,
		subdomain: form.subdomain || null,
		run_mode: form.run_mode,
	}),
)

function hydrate(meta: CoreMetadata | null) {
	if (!meta) return
	form.name = meta.name
	form.description = meta.description ?? ''
	form.banner = meta.banner ?? ''
	form.subdomain = meta.subdomain ?? ''
	form.run_mode = meta.run_mode
	setupMode.value = meta.setup_mode
}

async function onSave() {
	saved.value = false
	if (isPreviewConnected.value) {
		saved.value = true
		return
	}
	const result = await save()
	if (result) {
		hydrate(result)
		saved.value = true
	}
}

onMounted(async () => {
	if (isPreviewConnected.value) {
		hydrate({
			name: 'Amberite Core',
			description: isPreviewLocal.value ? 'Running on this computer.' : 'Linked through pairing.',
			banner: '',
			subdomain: 'amberite-home',
			setup_mode: isPreviewLocal.value ? 'local' : 'remote',
			run_mode: isPreviewLocal.value ? 'app_open' : 'manual',
		} as CoreMetadata)
		return
	}
	await load()
	hydrate(metadata.value)
})

watch(isPreviewConnected, (connected) => {
	if (!connected) return
	hydrate({
		name: 'Amberite Core',
		description: isPreviewLocal.value ? 'Running on this computer.' : 'Linked through pairing.',
		banner: '',
		subdomain: 'amberite-home',
		setup_mode: isPreviewLocal.value ? 'local' : 'remote',
		run_mode: isPreviewLocal.value ? 'app_open' : 'manual',
	} as CoreMetadata)
})
</script>

<template>
	<div class="flex w-full max-w-[min(928px,calc(95vw-10rem))] flex-col gap-5">
		<div v-if="loading && !isPreviewConnected" class="rounded-2xl bg-surface-3 p-5 text-secondary">Loading Core settings...</div>
		<p v-else-if="error && !isPreviewConnected" class="m-0 rounded-2xl bg-bg-red p-5 font-semibold text-contrast">Couldn't reach this Core: {{ error.message }}</p>

		<template v-else>
			<section class="overflow-hidden rounded-[28px] bg-surface-3">
				<div class="flex min-h-36 items-end bg-surface-4 bg-cover bg-center p-6" :style="form.banner ? { backgroundImage: `url(${form.banner})` } : {}">
					<div>
						<div class="mb-2 flex items-center gap-2 text-sm font-bold text-brand"><ServerStackIcon /> {{ isLocal ? 'Local' : 'Linked' }}</div>
						<h1 class="m-0 text-3xl font-black text-contrast">{{ form.name || 'Amberite Core' }}</h1>
						<p class="m-0 mt-2 text-secondary">{{ form.description || 'No description set.' }}</p>
					</div>
				</div>
				<div class="grid gap-4 p-5 md:grid-cols-2">
					<label class="flex flex-col gap-2">
						<span class="text-sm font-bold text-contrast">Core name</span>
						<StyledInput v-model="form.name" />
					</label>
					<label class="flex flex-col gap-2">
						<span class="text-sm font-bold text-contrast">Subdomain</span>
						<StyledInput v-model="form.subdomain" placeholder="family-core" />
					</label>
					<label class="flex flex-col gap-2 md:col-span-2">
						<span class="text-sm font-bold text-contrast">Description</span>
						<StyledInput v-model="form.description" multiline />
					</label>
					<label class="flex flex-col gap-2 md:col-span-2">
						<span class="text-sm font-bold text-contrast">Banner image URL</span>
						<StyledInput v-model="form.banner" placeholder="https://..." />
					</label>
				</div>
			</section>

			<section class="grid gap-4 md:grid-cols-2">
				<div class="rounded-2xl bg-surface-3 p-5">
					<h2 class="m-0 mb-4 text-xl font-bold text-contrast">Runtime</h2>
					<div class="flex flex-col gap-4">
						<div class="flex items-center justify-between gap-4">
							<div><div class="font-bold text-contrast">Start on startup</div><p class="m-0 text-sm text-secondary">Useful for a local Core host.</p></div>
							<Toggle v-model="startOnStartup" :disabled="!isLocal" />
						</div>
						<div class="flex items-center justify-between gap-4">
							<div><div class="font-bold text-contrast">Run in background</div><p class="m-0 text-sm text-secondary">Keep local Core alive outside the app.</p></div>
							<Toggle v-model="runInBackground" :disabled="!isLocal" />
						</div>
						<label class="flex flex-col gap-2">
							<span class="text-sm font-bold text-contrast">Run mode</span>
							<select v-model="form.run_mode" class="rounded-xl border-0 bg-surface-4 px-3 py-2">
								<option value="manual">Manual</option>
								<option value="app_open">When app opens</option>
								<option value="startup">On system startup</option>
							</select>
						</label>
					</div>
				</div>
				<div class="rounded-2xl bg-surface-3 p-5">
					<h2 class="m-0 mb-4 text-xl font-bold text-contrast">Danger zone</h2>
					<p class="m-0 mb-4 text-sm text-secondary">
						{{ isLocal ? 'Delete the local Core install UI state.' : 'Disconnect this app from the external Core.' }}
					</p>
					<ButtonStyled color="red">
						<button @click="dangerModal?.show()"><TrashIcon /> {{ isLocal ? 'Delete Core' : 'Disconnect Core' }}</button>
					</ButtonStyled>
				</div>
			</section>

			<div class="flex items-center gap-3">
				<ButtonStyled color="brand"><button :disabled="saving" @click="onSave"><SaveIcon /> {{ saving ? 'Saving...' : 'Save changes' }}</button></ButtonStyled>
				<span v-if="saved" class="text-sm font-bold text-green">Saved.</span>
				<span v-if="saveError" class="text-sm font-bold text-red">{{ saveError.message }}</span>
			</div>
		</template>

		<NewModal ref="dangerModal" :header="isLocal ? 'Delete local Core' : 'Disconnect Core'" fade="danger" max-width="520px">
			<div class="flex flex-col gap-4">
				<div class="flex gap-3 rounded-2xl bg-bg-red p-4">
					<TriangleAlertIcon class="shrink-0 text-red" />
					<p class="m-0 text-sm text-contrast">This is UI-only for now. Backend deletion/disconnect commands are not wired yet.</p>
				</div>
				<ButtonStyled color="red"><button disabled>{{ isLocal ? 'Delete Core' : 'Disconnect Core' }}</button></ButtonStyled>
			</div>
		</NewModal>
	</div>
</template>
