<template>
	<NewModal
		ref="modal"
		:header="formatMessage(messages.title)"
		max-width="500px"
		:on-hide="() => settle(false)"
	>
		<p class="m-0">
			{{ formatMessage(messages.body) }}
			<a :href="url" target="_blank" rel="noopener" class="text-link">{{
				formatMessage(messages.read)
			}}</a>
		</p>
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
import { CheckIcon, XIcon } from '@modrinth/assets'
import { ref } from 'vue'

import { Button } from '#ui/components/base/buttons'
import NewModal from '#ui/components/modal/NewModal.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { commonMessages } from '#ui/utils/common-messages'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: { id: 'servers.eula.title', defaultMessage: 'Minecraft EULA' },
	body: {
		id: 'servers.eula.body',
		defaultMessage:
			"To create servers on this Core, you need to agree to the Minecraft EULA. Your agreement covers every server on this Core, including ones you create later, and can't be undone.",
	},
	read: { id: 'servers.eula.read', defaultMessage: 'Read the EULA' },
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
