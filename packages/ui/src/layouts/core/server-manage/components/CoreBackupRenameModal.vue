<template>
	<NewModal ref="modal" header="Rename backup" width="500px" @show="focusInput">
		<div class="-mb-2 flex flex-col gap-2">
			<label for="core-backup-rename-input">
				<span class="text-lg font-semibold text-contrast">Name</span>
			</label>
			<StyledInput
				id="core-backup-rename-input"
				ref="input"
				v-model="backupName"
				:maxlength="48"
				wrapper-class="w-full"
			/>
		</div>

		<template #actions>
			<div class="flex justify-end gap-2">
				<ButtonStyled type="outlined">
					<button @click="hide">
						<XIcon />
						Cancel
					</button>
				</ButtonStyled>
				<ButtonStyled color="brand">
					<button :disabled="saving || !currentBackup || !backupName.trim()" @click="renameBackup">
						<LoaderCircleIcon v-if="saving" class="animate-spin" />
						<EditIcon v-else />
						{{ saving ? 'Renaming...' : 'Rename backup' }}
					</button>
				</ButtonStyled>
			</div>
		</template>
	</NewModal>
</template>

<script setup lang="ts">
import type { CoreBackup } from '@amberite/amberite-api'
import { EditIcon, LoaderCircleIcon, XIcon } from '@modrinth/assets'
import { nextTick, ref } from 'vue'

import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import StyledInput from '#ui/components/base/StyledInput.vue'
import NewModal from '#ui/components/modal/NewModal.vue'

defineProps<{ saving?: boolean }>()
const emit = defineEmits<{ rename: [backup: CoreBackup, name: string] }>()

const modal = ref<InstanceType<typeof NewModal>>()
const input = ref<HTMLInputElement>()
const currentBackup = ref<CoreBackup | null>(null)
const backupName = ref('')

function focusInput() {
	nextTick(() => setTimeout(() => input.value?.focus(), 100))
}

function show(backup: CoreBackup) {
	currentBackup.value = backup
	backupName.value = backup.name
	modal.value?.show()
}

function hide() {
	modal.value?.hide()
}

function renameBackup() {
	if (!currentBackup.value) return
	emit('rename', currentBackup.value, backupName.value.trim())
}

defineExpose({ show, hide })
</script>
