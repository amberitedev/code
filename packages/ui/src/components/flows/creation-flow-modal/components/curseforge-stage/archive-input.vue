<template>
	<div
		v-if="file"
		class="flex h-10 min-w-0 items-center gap-2 rounded-[14px] border border-solid border-surface-5 bg-surface-4 px-4"
	>
		<FileIcon class="size-5 shrink-0 text-primary" aria-hidden="true" />
		<span class="min-w-0 flex-1 truncate font-semibold text-contrast" :title="file.name">
			{{ file.name }}
		</span>
		<IconButton
			type="quiet"
			size="sm"
			:disabled="disabled"
			:label="formatMessage(messages.removeFile, { filename: file.name })"
			@click="file = null"
		>
			<XIcon aria-hidden="true" />
		</IconButton>
	</div>
	<DropzoneFileInput
		v-else
		v-model:processing="processing"
		size="small"
		:accept="accept"
		:multiple="false"
		:local-files="localFiles"
		:primary-prompt="null"
		:secondary-prompt="formatMessage(messages.uploadPrompt)"
		:processing-prompt="formatMessage(messages.processingFile)"
		class="!rounded-[20px] !border-[1.5px] !border-surface-4 !bg-surface-2 hover:brightness-110 focus-within:outline focus-within:outline-2 focus-within:outline-brand data-[highlighted]:!border-brand data-[highlighted]:!bg-brand-highlight"
		@change="file = $event[0] ?? null"
		@local-change="file = $event[0] ?? null"
	/>
</template>

<script setup lang="ts">
import type { LocalFile } from '@modrinth/api-client'
import { FileIcon, XIcon } from '@modrinth/assets'

import { IconButton } from '#ui/components/base/buttons'
import DropzoneFileInput from '#ui/components/base/DropzoneFileInput.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'

const {
	accept = '.zip',
	localFiles = true,
	disabled = false,
} = defineProps<{
	accept?: string
	localFiles?: boolean
	disabled?: boolean
}>()

/** A file picked in the browser, or dropped from disk in the App. */
const file = defineModel<File | LocalFile | null>({ required: true })
const processing = defineModel<boolean>('processing', { default: false })
const { formatMessage } = useVIntl()

const messages = defineMessages({
	uploadPrompt: {
		id: 'creation-flow.curseforge.upload-prompt',
		defaultMessage: 'Drag and drop file or click to browse',
	},
	processingFile: {
		id: 'creation-flow.curseforge.processing-file',
		defaultMessage: 'Processing...',
	},
	removeFile: {
		id: 'creation-flow.curseforge.remove-file',
		defaultMessage: 'Remove {filename}',
	},
})
</script>
