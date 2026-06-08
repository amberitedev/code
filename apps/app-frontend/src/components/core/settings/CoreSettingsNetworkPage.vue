<script setup lang="ts">
import { SettingsLabel, StyledInput, Toggle, UnsavedChangesPopup } from '@modrinth/ui'
import { useStorage } from '@vueuse/core'
import { computed, ref, watch } from 'vue'

import { useSocial } from '@/composables/useSocial'

const social = useSocial()
const group = computed(() => social.group.value?.group)
const core = computed(() => social.group.value?.core)

const connectionUrl = ref('')
const subdomain = ref('')
const savedLocalSettings = useStorage('amberite-core-network-settings', {
	requirePairingCodes: true,
	advertiseOnLan: false,
})
const requirePairingCodes = ref(savedLocalSettings.value.requirePairingCodes)
const advertiseOnLan = ref(savedLocalSettings.value.advertiseOnLan)
const isUpdating = ref(false)
const savedNetwork = computed(() => ({
	subdomain: group.value?.subdomain ?? '',
	requirePairingCodes: savedLocalSettings.value.requirePairingCodes,
	advertiseOnLan: savedLocalSettings.value.advertiseOnLan,
}))
const modifiedNetwork = computed(() => ({
	subdomain: subdomain.value,
	requirePairingCodes: requirePairingCodes.value,
	advertiseOnLan: advertiseOnLan.value,
}))

watch(
	() => social.group.value,
	(next) => {
		connectionUrl.value = next?.core?.connectionUrl ?? ''
		subdomain.value = next?.group.subdomain ?? ''
	},
	{ immediate: true },
)

const canSave = computed(() => social.canManage.value && !!group.value)
async function saveNetwork() {
	if (!canSave.value) return
	isUpdating.value = true
	try {
		await social.updateGroup({ subdomain: subdomain.value.trim() || undefined })
		savedLocalSettings.value = {
			requirePairingCodes: requirePairingCodes.value,
			advertiseOnLan: advertiseOnLan.value,
		}
	} finally {
		isUpdating.value = false
	}
}

function resetNetwork() {
	subdomain.value = group.value?.subdomain ?? ''
	requirePairingCodes.value = savedLocalSettings.value.requirePairingCodes
	advertiseOnLan.value = savedLocalSettings.value.advertiseOnLan
}
</script>

<template>
	<div class="relative h-full w-full">
		<section class="flex flex-col gap-3">
			<SettingsLabel
				title="Networking"
				description="Connection details for this app and invite links."
			/>
			<div class="flex flex-col gap-3">
				<div>
					<SettingsLabel
						id="core-connection-url"
						title="Core API address"
						description="Where this app connects to your Core manager."
					/>
					<StyledInput
						id="core-connection-url"
						v-model="connectionUrl"
						placeholder="http://localhost:16662"
						readonly
					/>
				</div>
				<div>
					<SettingsLabel
						id="core-subdomain"
						title="Public subdomain"
						description="Optional short name used for friend group invite links."
					/>
					<StyledInput
						id="core-subdomain"
						v-model="subdomain"
						placeholder="your-group"
						:disabled="!canSave"
					/>
				</div>
			</div>
		</section>

		<section class="mt-6 flex flex-col gap-3">
			<SettingsLabel title="Pairing" description="Temporary local-only defaults for future pairing flows." />
			<div class="flex items-center justify-between gap-4">
				<label class="flex flex-col gap-1">
					<p class="m-0 font-bold text-contrast">Require pairing codes</p>
					<p class="m-0 text-sm text-secondary">New devices need an invite or pairing code.</p>
				</label>
				<Toggle v-model="requirePairingCodes" />
			</div>
			<div class="flex items-center justify-between gap-4">
				<label class="flex flex-col gap-1">
					<p class="m-0 font-bold text-contrast">Advertise on local network</p>
					<p class="m-0 text-sm text-secondary">Let trusted devices discover this Core on LAN.</p>
				</label>
				<Toggle v-model="advertiseOnLan" />
			</div>
		</section>

		<section class="mt-6 flex flex-col gap-2.5 pb-10">
			<SettingsLabel title="Info" />
			<div class="flex flex-col gap-2.5 rounded-xl bg-surface-2 p-4">
				<div class="flex items-start justify-between gap-4">
					<span class="mt-1">Current host</span>
					<span class="break-all text-right text-sm">
						{{ core?.connectionUrl || 'No Core connection saved' }}
					</span>
				</div>
			</div>
		</section>
		<UnsavedChangesPopup
			v-if="canSave"
			:original="savedNetwork"
			:modified="modifiedNetwork"
			:saving="isUpdating || social.loading.value"
			@save="saveNetwork"
			@reset="resetNetwork"
		/>
	</div>
</template>
