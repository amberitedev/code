<template>
	<NewModal ref="modal" header="Restore backup" fade="danger" width="500px">
		<div class="flex flex-col gap-6">
			<Admonition v-if="isServerRunning" type="critical" header="Server is running">
				Stop the server before restoring a backup.
			</Admonition>
			<Admonition v-else type="critical" header="Your server files will be replaced">
				Restoring your server will replace the current server files. Any changes made since that
				backup will be permanently lost.
			</Admonition>

			<div v-if="currentBackup" class="flex flex-col gap-2">
				<span class="font-semibold text-contrast">Backup</span>
				<CoreBackupItem :backup="currentBackup" preview class="!bg-surface-2 !shadow-none" />
			</div>
		</div>

		<template #actions>
			<div class="flex justify-end gap-2">
				<ButtonStyled type="outlined">
					<button @click="hide">
						<XIcon />
						Cancel
					</button>
				</ButtonStyled>
				<ButtonStyled color="red">
					<button :disabled="restoring || isServerRunning || !currentBackup" @click="restoreBackup">
						<LoaderCircleIcon v-if="restoring" class="animate-spin" />
						<RotateCounterClockwiseIcon v-else />
						{{ restoring ? 'Restoring...' : 'Restore backup' }}
					</button>
				</ButtonStyled>
			</div>
		</template>
	</NewModal>
</template>

<script setup lang="ts">
import type { CoreBackup } from '@amberite/amberite-api'
import { LoaderCircleIcon, RotateCounterClockwiseIcon, XIcon } from '@modrinth/assets'
import { ref } from 'vue'

import Admonition from '#ui/components/base/Admonition.vue'
import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import NewModal from '#ui/components/modal/NewModal.vue'

import CoreBackupItem from './CoreBackupItem.vue'

defineProps<{ isServerRunning: boolean; restoring?: boolean }>()
const emit = defineEmits<{ restore: [backup: CoreBackup] }>()

const modal = ref<InstanceType<typeof NewModal>>()
const currentBackup = ref<CoreBackup | null>(null)

function show(backup: CoreBackup) {
	currentBackup.value = backup
	modal.value?.show()
}

function hide() {
	modal.value?.hide()
}

function restoreBackup() {
	if (currentBackup.value) emit('restore', currentBackup.value)
}

defineExpose({ show, hide })
</script>
