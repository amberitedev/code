<template>
	<div class="flex flex-col gap-6">
		<header>
			<h3 class="m-0 text-lg font-extrabold text-contrast">Installation</h3>
			<p class="m-0 mt-1 text-sm text-secondary">
				Change the Minecraft version or loader. Applying reinstalls the server.
			</p>
		</header>

		<label class="flex max-w-xs flex-col gap-1">
			<span class="text-sm font-bold text-contrast">Loader</span>
			<Combobox v-model="form.loader" name="Loader" :options="loaderOptions" />
		</label>

		<label class="flex max-w-xs flex-col gap-1">
			<span class="text-sm font-bold text-contrast">Minecraft version</span>
			<StyledInput v-model="form.gameVersion" type="text" placeholder="1.21.1" />
		</label>

		<label v-if="form.loader !== 'vanilla'" class="flex max-w-xs flex-col gap-1">
			<span class="text-sm font-bold text-contrast">Loader version</span>
			<StyledInput v-model="form.loaderVersion" type="text" placeholder="Latest (leave blank)" />
		</label>

		<div>
			<ButtonStyled color="brand">
				<button type="button" :disabled="!isDirty || applying" @click="apply">
					<SpinnerIcon v-if="applying" class="animate-spin" />
					<DownloadIcon v-else />
					Apply and reinstall
				</button>
			</ButtonStyled>
		</div>

		<hr class="m-0 border-0 border-t border-solid border-divider" />

		<div class="flex flex-col gap-2">
			<h4 class="m-0 text-base font-bold text-contrast">Repair</h4>
			<p class="m-0 text-sm text-secondary">
				Reinstall the current version's files without changing the version. Use this if the server
				is broken.
			</p>
			<div>
				<ButtonStyled color="orange">
					<button type="button" :disabled="repairing" @click="runRepair">
						<SpinnerIcon v-if="repairing" class="animate-spin" />
						<WrenchIcon v-else />
						Repair server
					</button>
				</ButtonStyled>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import type { CoreChangeVersionBody, CoreModLoader } from '@amberite/amberite-api'
import { DownloadIcon, SpinnerIcon, WrenchIcon } from '@modrinth/assets'
import { ButtonStyled, Combobox, StyledInput } from '@modrinth/ui'
import { computed, reactive, ref } from 'vue'

import { useServerSettings } from '../use-server-settings'

const { core, instanceId, changeVersion, repairServer } = useServerSettings()

const loaderOptions = [
	{ value: 'vanilla', label: 'Vanilla' },
	{ value: 'paper', label: 'Paper' },
	{ value: 'fabric', label: 'Fabric' },
	{ value: 'forge', label: 'Forge' },
	{ value: 'neoforge', label: 'NeoForge' },
	{ value: 'quilt', label: 'Quilt' },
]

type InstallForm = { loader: CoreModLoader; gameVersion: string; loaderVersion: string }

const form = reactive<InstallForm>({ loader: 'vanilla', gameVersion: '', loaderVersion: '' })
let saved: InstallForm = { ...form }
const applying = ref(false)
const repairing = ref(false)

async function load() {
	const instance = await core.getInstance(instanceId.value)
	Object.assign(form, {
		loader: instance.loader,
		gameVersion: instance.game_version,
		loaderVersion: instance.loader_version ?? '',
	})
	saved = { ...form }
}

const isDirty = computed(
	() =>
		form.loader !== saved.loader ||
		form.gameVersion !== saved.gameVersion ||
		form.loaderVersion !== saved.loaderVersion,
)

async function apply() {
	const body: CoreChangeVersionBody = {}
	if (form.gameVersion !== saved.gameVersion) body.game_version = form.gameVersion.trim()
	if (form.loader !== saved.loader) body.loader = form.loader
	if (form.loaderVersion !== saved.loaderVersion) {
		body.loader_version = form.loaderVersion.trim() || null
	}
	applying.value = true
	try {
		await changeVersion(body)
		await load()
	} finally {
		applying.value = false
	}
}

async function runRepair() {
	repairing.value = true
	try {
		await repairServer()
	} finally {
		repairing.value = false
	}
}

await load()
</script>
