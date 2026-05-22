<template>
	<NewModal ref="modal" header="Delete backup" fade="danger" width="500px">
		<Admonition type="critical" header="This cannot be undone">
			The selected backup will be permanently deleted.
		</Admonition>

		<template #actions>
			<div class="flex justify-end gap-2">
				<ButtonStyled type="outlined">
					<button @click="hide">
						<XIcon />
						Cancel
					</button>
				</ButtonStyled>
				<ButtonStyled color="red">
					<button :disabled="deleting || !currentBackup" @click="deleteBackup">
						<LoaderCircleIcon v-if="deleting" class="animate-spin" />
						<TrashIcon v-else />
						{{ deleting ? 'Deleting...' : 'Delete backup' }}
					</button>
				</ButtonStyled>
			</div>
		</template>
	</NewModal>
</template>

<script setup lang="ts">
import type { CoreBackup } from '@amberite/amberite-api'
import { LoaderCircleIcon, TrashIcon, XIcon } from '@modrinth/assets'
import { ref } from 'vue'

import Admonition from '#ui/components/base/Admonition.vue'
import ButtonStyled from '#ui/components/base/ButtonStyled.vue'
import NewModal from '#ui/components/modal/NewModal.vue'

defineProps<{ deleting?: boolean }>()
const emit = defineEmits<{ delete: [backup: CoreBackup] }>()

const modal = ref<InstanceType<typeof NewModal>>()
const currentBackup = ref<CoreBackup | null>(null)

function show(backup: CoreBackup) {
	currentBackup.value = backup
	modal.value?.show()
}

function hide() {
	modal.value?.hide()
}

function deleteBackup() {
	if (currentBackup.value) emit('delete', currentBackup.value)
}

defineExpose({ show, hide })
</script>
