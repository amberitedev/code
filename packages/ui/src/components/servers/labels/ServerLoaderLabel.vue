<template>
	<div class="flex min-w-0 flex-row items-center gap-2 truncate">
		<Separator v-if="!noSeparator" />
		<div class="flex flex-row items-center gap-1.5">
			<TagIcon v-if="loader" :tag="loader" enforce-type="loader" />
			<AutoLink
				v-if="isLink"
				v-tooltip="'Change server loader'"
				:to="settingsLinkTarget"
				class="flex min-w-0 items-center font-medium text-sm"
				:class="settingsLinkTarget ? 'hover:underline' : ''"
			>
				<span v-if="loader">
					{{ loader }}
					<span v-if="loaderVersion">{{ loaderVersion }}</span>
				</span>
			</AutoLink>
			<div v-else class="pointer-events-none min-w-0 font-medium text-sm">
				<span v-if="loader">
					{{ loader }}
					<span v-if="loaderVersion">{{ loaderVersion }}</span>
				</span>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

import { injectServerSettingsModal } from '#ui/providers/server-settings-modal'
import type { ServerLoader } from '#ui/utils/loaders'

import AutoLink from '../../base/AutoLink.vue'
import TagIcon from '../../base/TagIcon.vue'
import Separator from './Separator.vue'

defineProps<{
	noSeparator?: boolean
	loader?: ServerLoader
	loaderVersion?: string
	isLink?: boolean
}>()

const settingsModal = injectServerSettingsModal(null)
const settingsLinkTarget = computed(() => {
	if (settingsModal) {
		return () => settingsModal.openServerSettings({ tabId: 'installation' })
	}
	return ''
})
</script>
