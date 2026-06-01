<script setup lang="ts">
import { ChevronRightIcon } from '@modrinth/assets'
import {
	commonMessages,
	defineMessage,
	TabbedModal,
	type TabbedModalTab,
	useVIntl,
} from '@modrinth/ui'
import { computed, nextTick, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import { injectCoreServerContext } from '../core-server-instance'
import advanced from './pages/advanced.vue'
import general from './pages/general.vue'
import installation from './pages/installation.vue'
import network from './pages/network.vue'
import properties from './pages/properties.vue'
import { provideServerSettings } from './server-settings'
import { injectServerSettingsController } from './server-settings-controller'
import { serverSettingsTabDefinitions, type ServerSettingsTabId } from './tabs'

const { formatMessage } = useVIntl()
const router = useRouter()
const { server } = injectCoreServerContext()

const controller = injectServerSettingsController()

const modal = ref<InstanceType<typeof TabbedModal> | null>(null)

const currentUserId = ref<string | null>('local')
const currentUserRole = ref<string | null>(null)
const isApp = ref(true)

const serverSettingsTabComponentMap = {
	general,
	installation,
	network,
	properties,
	advanced,
} as const

provideServerSettings({
	isApp,
	currentUserId,
	currentUserRole,
	browseModpacks: () => {
		controller.close()
		void router.push('/browse/modpack')
	},
	closeModal: () => controller.close(),
})

const ownerId = computed(() => server.value?.owner_id ?? 'local')
const isOwner = computed(() => currentUserId.value != null && currentUserId.value === ownerId.value)
const isAdmin = computed(() => currentUserRole.value === 'admin')

const tabs = computed<TabbedModalTab[]>(() =>
	serverSettingsTabDefinitions.map((tab) => {
		const ctx = {
			serverId: server.value?.server_id ?? '',
			ownerId: ownerId.value,
			serverStatus: server.value?.status,
			isOwner: isOwner.value,
			isAdmin: isAdmin.value,
		}
		return {
			name: defineMessage({
				id: `server.settings.tabs.${tab.id}`,
				defaultMessage: tab.label,
			}),
			icon: tab.icon,
			content: serverSettingsTabComponentMap[tab.id],
			shown: tab.shown ? tab.shown(ctx) : true,
		}
	}),
)

function syncTab(tabId: ServerSettingsTabId) {
	const fullIndex = serverSettingsTabDefinitions.findIndex((d) => d.id === tabId)
	if (fullIndex < 0 || tabs.value[fullIndex]?.shown === false) return
	let visibleIndex = 0
	for (let i = 0; i < fullIndex; i++) {
		if (tabs.value[i]?.shown !== false) visibleIndex++
	}
	nextTick(() => modal.value?.setTab(visibleIndex))
}

watch(
	() => controller.isOpen.value,
	(open) => {
		if (open) {
			modal.value?.show()
			syncTab(controller.activeTab.value)
		} else {
			modal.value?.hide()
		}
	},
)

watch(
	() => controller.activeTab.value,
	(tabId) => {
		if (controller.isOpen.value) syncTab(tabId)
	},
)
</script>

<template>
	<TabbedModal
		ref="modal"
		:tabs="tabs"
		:max-width="'min(980px, calc(95vw - 2rem))'"
		:width="'min(980px, calc(95vw - 2rem))'"
		:on-hide="() => controller.close()"
	>
		<template #title>
			<span class="flex items-center gap-2 text-lg font-semibold text-primary">
				{{ server?.name || 'Server' }} <ChevronRightIcon />
				<span class="font-extrabold text-contrast">{{
					formatMessage(commonMessages.settingsLabel)
				}}</span>
			</span>
		</template>
	</TabbedModal>
</template>
