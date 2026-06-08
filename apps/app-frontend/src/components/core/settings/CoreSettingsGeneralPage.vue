<script setup lang="ts">
import { IconSelect, SettingsLabel, StyledInput, UnsavedChangesPopup } from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, ref, watch } from 'vue'

import { useSocial } from '@/composables/useSocial'

const social = useSocial()
const group = computed(() => social.group.value?.group)
const core = computed(() => social.group.value?.core)

const groupName = ref('')
const iconUrl = ref('')
const description = ref('')
const isUpdating = ref(false)
const savedGeneral = computed(() => ({
	name: group.value?.name ?? '',
	description: group.value?.description ?? '',
	banner: group.value?.banner,
}))
const modifiedGeneral = computed(() => ({
	name: groupName.value,
	description: description.value,
	banner: iconUrl.value,
}))

watch(
	group,
	(next) => {
		groupName.value = next?.name ?? ''
		description.value = next?.description ?? ''
		iconUrl.value = next?.banner
	},
	{ immediate: true },
)

const canSave = computed(() => social.canManage.value && !!group.value)
async function saveGeneral() {
	if (!canSave.value) return
	isUpdating.value = true
	try {
		await social.updateGroup({
			name: groupName.value.trim() || undefined,
			description: description.value.trim() || undefined,
			banner: iconUrl.value,
		})
	} finally {
		isUpdating.value = false
	}
}

function resetGeneral() {
	groupName.value = group.value?.name ?? ''
	description.value = group.value?.description ?? ''
	iconUrl.value = group.value?.banner
}

async function selectIcon() {
	const value = await open({
		multiple: false,
		filters: [{ name: 'Image', extensions: ['png', 'jpeg', 'svg', 'webp', 'gif', 'jpg'] }],
	})
	if (typeof value === 'string') iconUrl.value = convertFileSrc(value)
}
</script>

<template>
	<div class="relative h-full w-full">
		<section class="flex flex-col gap-4">
			<div>
				<SettingsLabel
					title="General"
					description="Basic friend group details shown around Core access and invites."
				/>
				<div class="grid gap-4 md:grid-cols-[minmax(0,1fr)_8rem]">
					<div class="flex flex-col gap-3">
						<StyledInput
							id="core-group-name"
							v-model="groupName"
							placeholder="Friend group name"
							:disabled="!canSave"
						/>
						<StyledInput
							v-model="description"
							placeholder="Description"
							multiline
							:rows="5"
							:disabled="!canSave"
						/>
					</div>
					<div class="flex flex-col gap-2">
						<span class="font-semibold text-contrast">Icon</span>
						<IconSelect
							v-model="iconUrl"
							:options="['select', 'remove']"
							:show-edit-icon="false"
							@select="selectIcon"
							@remove="iconUrl = undefined"
						/>
					</div>
				</div>
			</div>
		</section>

		<section class="mt-6 flex flex-col gap-2.5 pb-10">
			<SettingsLabel title="Info" description="Read-only Core details for this app connection." />
			<div class="flex flex-col gap-2.5 rounded-xl bg-surface-2 p-4">
				<div class="flex items-start justify-between gap-4">
					<span class="mt-1">Connection URL</span>
					<span class="break-all text-right text-sm">{{ core?.connectionUrl || 'Not connected' }}</span>
				</div>
				<div class="flex items-start justify-between gap-4">
					<span class="mt-1">Core ID</span>
					<span class="break-all text-right text-sm">{{ core?.id || group?.coreId || 'Unknown' }}</span>
				</div>
			</div>
		</section>
		<UnsavedChangesPopup
			v-if="canSave"
			:original="savedGeneral"
			:modified="modifiedGeneral"
			:saving="isUpdating || social.loading.value"
			@save="saveGeneral"
			@reset="resetGeneral"
		/>
	</div>
</template>
