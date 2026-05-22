<template>
	<NewModal ref="modal" header="Create backup" width="500px" @show="focusInput">
		<div class="-mb-2 flex flex-col gap-2">
			<label for="core-backup-name-input">
				<span class="text-lg font-semibold text-contrast">Name</span>
			</label>
			<StyledInput
				id="core-backup-name-input"
				ref="input"
				v-model="backupName"
				:placeholder="`Backup #${newBackupAmount}`"
				:maxlength="48"
				wrapper-class="w-full"
			/>
			<Transition
				enter-active-class="transition-all duration-300 ease-out"
				enter-from-class="opacity-0 max-h-0"
				enter-to-class="opacity-100 max-h-20"
				leave-active-class="transition-all duration-200 ease-in"
				leave-from-class="opacity-100 max-h-20"
				leave-to-class="opacity-0 max-h-0"
			>
				<div v-if="nameExists" class="mt-2 flex items-center gap-1 overflow-hidden">
					<IssuesIcon class="hidden text-orange sm:block" />
					<span class="text-sm text-orange">
						You already have a backup named '<span class="font-semibold">{{ trimmedName }}</span
						>'
					</span>
				</div>
			</Transition>
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
					<button :disabled="creating || nameExists" @click="createBackup">
						<LoaderCircleIcon v-if="creating" class="animate-spin" />
						<PlusIcon v-else />
						{{ creating ? 'Creating...' : 'Create backup' }}
					</button>
				</ButtonStyled>
			</div>
		</template>
	</NewModal>
</template>

<script setup lang="ts">
import type { CoreBackup } from '@amberite/amberite-api'
import { IssuesIcon, LoaderCircleIcon, PlusIcon, XIcon } from '@modrinth/assets'
import { computed, nextTick, ref } from 'vue'

import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import StyledInput from '#ui/components/base/StyledInput.vue'
import NewModal from '#ui/components/modal/NewModal.vue'

const props = defineProps<{ backups: CoreBackup[]; creating?: boolean }>()
const emit = defineEmits<{ create: [name: string] }>()

const modal = ref<InstanceType<typeof NewModal>>()
const input = ref<HTMLInputElement>()
const backupName = ref('')
const newBackupAmount = computed(() => props.backups.length + 1)
const trimmedName = computed(() => backupName.value.trim())
const nameExists = computed(() =>
	props.backups.some(
		(backup) => backup.name.trim().toLowerCase() === trimmedName.value.toLowerCase(),
	),
)

function focusInput() {
	nextTick(() => setTimeout(() => input.value?.focus(), 100))
}

function show() {
	backupName.value = ''
	modal.value?.show()
}

function hide() {
	modal.value?.hide()
}

function createBackup() {
	emit('create', trimmedName.value || `Backup #${newBackupAmount.value}`)
}

defineExpose({ show, hide })
</script>
