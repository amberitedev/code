<template>
	<NewModal
		ref="modal"
		:header="formatMessage(messages.title)"
		max-width="520px"
		:on-hide="() => settle(false)"
	>
		<div class="flex flex-col gap-3">
			<p class="m-0">{{ formatMessage(messages.lead) }}</p>
			<a :href="url" target="_blank" rel="noopener" class="flex w-fit items-center gap-1 text-link">
				{{ formatMessage(messages.read) }}
				<ExternalIcon class="size-4" />
			</a>
		</div>
		<template #actions>
			<div class="flex justify-end gap-2">
				<Button type="outlined" @click="modal?.hide()">
					<XIcon />
					{{ formatMessage(commonMessages.cancelButton) }}
				</Button>
				<Button type="colored" color="brand" @click="accept">
					<CheckIcon />
					{{ formatMessage(messages.accept) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>

<script setup lang="ts">
import { CheckIcon, ExternalIcon, XIcon } from '@modrinth/assets'
import { ref } from 'vue'

import { Button } from '#ui/components/base/buttons'
import NewModal from '#ui/components/modal/NewModal.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { commonMessages } from '#ui/utils/common-messages'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: { id: 'servers.eula.title', defaultMessage: 'Minecraft EULA' },
	lead: {
		id: 'servers.eula.lead',
		defaultMessage: 'Agree to the EULA to create servers on this Core.',
	},
	read: { id: 'servers.eula.read', defaultMessage: 'Read the Minecraft EULA' },
	accept: { id: 'servers.eula.accept', defaultMessage: 'I agree to the EULA' },
})

const modal = ref<InstanceType<typeof NewModal>>()
const url = ref('')
let resolve: ((accepted: boolean) => void) | null = null

function settle(accepted: boolean) {
	resolve?.(accepted)
	resolve = null
}

/** Shows the EULA at `eulaUrl`. Resolves true when the owner accepts it. */
function ask(eulaUrl: string): Promise<boolean> {
	settle(false)
	url.value = eulaUrl
	return new Promise((done) => {
		resolve = done
		modal.value?.show()
	})
}

function accept() {
	settle(true)
	modal.value?.hide()
}

defineExpose({ ask })
</script>
