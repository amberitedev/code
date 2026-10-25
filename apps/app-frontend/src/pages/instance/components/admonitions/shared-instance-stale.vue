<template>
	<Admonition
		type="info"
		inline-actions
		:header="formatMessage(messages.sharedInstanceChangesHeader)"
		:progress="isPublishing ? publishProgress : undefined"
		:waiting="isPublishing && !publishTask?.totalBytes"
	>
		{{ formatMessage(messages.sharedInstanceChangesBody) }}
		<template #actions>
			<InstancePushButton :instance="instance" primary @published="emit('published')" />
		</template>
	</Admonition>
</template>

<script setup lang="ts">
import { Admonition, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import { findRunningSharedInstancePublish } from '@/components/ui/download-manager/shared-instance-publishes'
import InstancePushButton from '@/components/ui/linked-servers/InstancePushButton.vue'
import type { GameInstance } from '@/helpers/types'

import { instanceAdmonitionsMessages as messages } from './messages'

const props = defineProps<{
	instance: GameInstance
}>()

const emit = defineEmits<{
	published: []
}>()

const { formatMessage } = useVIntl()
// The push itself runs in InstancePushButton; the banner shows the upload while it runs.
const publishTask = computed(() => findRunningSharedInstancePublish(props.instance.id))
const isPublishing = computed(() => !!publishTask.value)
const publishProgress = computed(() =>
	publishTask.value?.totalBytes
		? publishTask.value.uploadedBytes / publishTask.value.totalBytes
		: 0,
)
</script>
