<template>
	<div class="flex flex-col gap-6">
		<div
			v-if="ctx.setupType.value === 'modpack' && ctx.modpackFile.value"
			class="flex flex-col gap-2"
		>
			<span class="font-semibold text-contrast">{{ formatMessage(messages.mrpackFileLabel) }}</span>
			<ArchiveInput :model-value="ctx.modpackFile.value" disabled />
		</div>
		<ModpackUploadProgress v-if="ctx.uploadProgress.value" :progress="ctx.uploadProgress.value" />
	</div>
</template>

<script setup lang="ts">
import { defineMessages, useVIntl } from '#ui/composables/i18n'

import { injectCreationFlowContext } from '../creation-flow-context'
import ArchiveInput from './curseforge-stage/archive-input.vue'
import ModpackUploadProgress from './ModpackUploadProgress.vue'

const ctx = injectCreationFlowContext()
const { formatMessage } = useVIntl()

const messages = defineMessages({
	mrpackFileLabel: {
		id: 'creation-flow.modal.uploading.mrpack-file-label',
		defaultMessage: '.mrpack file',
	},
})
</script>
