<template>
	<NewModal
		ref="modal"
		:header="formatMessage(messages.title)"
		max-width="520px"
		:on-hide="() => settle(false)"
	>
		<div class="flex flex-col gap-5">
			<div class="flex items-center gap-4">
				<ScaleIcon class="size-10 shrink-0 text-brand" />
				<p class="m-0 text-lg font-semibold text-contrast">{{ formatMessage(messages.lead) }}</p>
			</div>
			<ul class="m-0 flex list-none flex-col gap-3 p-0">
				<li class="flex items-center gap-3">
					<ServerStackIcon class="size-5 shrink-0 text-secondary" />
					{{ formatMessage(messages.covers) }}
				</li>
				<li class="flex items-center gap-3">
					<LockIcon class="size-5 shrink-0 text-secondary" />
					{{ formatMessage(messages.final) }}
				</li>
			</ul>
			<a
				:href="url"
				target="_blank"
				rel="noopener"
				class="flex w-fit items-center gap-2 font-semibold text-link"
			>
				<BookTextIcon class="size-5" />
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
import {
	BookTextIcon,
	CheckIcon,
	ExternalIcon,
	LockIcon,
	ScaleIcon,
	ServerStackIcon,
	XIcon,
} from '@modrinth/assets'
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
	covers: {
		id: 'servers.eula.covers',
		defaultMessage: 'Covers every server on this Core, including new ones',
	},
	final: { id: 'servers.eula.final', defaultMessage: "Can't be undone" },
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
