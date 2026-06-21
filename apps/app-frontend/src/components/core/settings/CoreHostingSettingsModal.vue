<script setup lang="ts">
import { GlobeIcon, InfoIcon, SettingsIcon, UsersIcon, WrenchIcon } from '@modrinth/assets'
import { TabbedModal } from '@modrinth/ui'
import { ref } from 'vue'

import CoreSettingsAdvancedPage from './CoreSettingsAdvancedPage.vue'
import CoreSettingsGeneralPage from './CoreSettingsGeneralPage.vue'
import CoreSettingsMembersPage from './CoreSettingsMembersPage.vue'
import CoreSettingsNetworkPage from './CoreSettingsNetworkPage.vue'

const modal = ref<InstanceType<typeof TabbedModal> | null>(null)

const tabs = [
	{
		name: { id: 'app.core.settings.tabs.general', defaultMessage: 'General' },
		icon: InfoIcon,
		content: CoreSettingsGeneralPage,
	},
	{
		name: { id: 'app.core.settings.tabs.members', defaultMessage: 'Default access' },
		icon: UsersIcon,
		content: CoreSettingsMembersPage,
	},
	{
		name: { id: 'app.core.settings.tabs.network', defaultMessage: 'Network' },
		icon: GlobeIcon,
		content: CoreSettingsNetworkPage,
	},
	{
		name: { id: 'app.core.settings.tabs.advanced', defaultMessage: 'Advanced' },
		icon: WrenchIcon,
		content: CoreSettingsAdvancedPage,
	},
]

function show() {
	modal.value?.show()
}

function hide() {
	modal.value?.hide()
}

defineExpose({ show, hide })
</script>

<template>
	<TabbedModal
		ref="modal"
		:tabs="tabs"
		:max-width="'min(980px, calc(95vw - 2rem))'"
		:width="'min(980px, calc(95vw - 2rem))'"
	>
		<template #title>
			<span class="flex items-center gap-2 text-lg font-extrabold text-contrast">
				<SettingsIcon /> Core settings
			</span>
		</template>
	</TabbedModal>
</template>
