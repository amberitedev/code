<script setup lang="ts">
import { core as coreRoutes } from '@amberite/contracts/core'
import { ChevronRightIcon } from '@modrinth/assets'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'

import type { TabbedModalTab } from '#ui/components'
import { TabbedModal } from '#ui/components'
import { defineMessage, defineMessages, useVIntl } from '#ui/composables/i18n'
import { serverKeys } from '#ui/composables/server-queries'
import {
	ServerSettingsAdvancedPage,
	ServerSettingsBackupsPage,
	ServerSettingsGeneralPage,
	ServerSettingsInstallationPage,
	ServerSettingsPropertiesPage,
	ServerSettingsSharingPage,
	serverSettingsTabDefinitions,
	type ServerSettingsTabId,
} from '#ui/layouts/shared/server-settings'
import { provideServerSettings } from '#ui/layouts/shared/server-settings/providers/server-settings'
import {
	injectModrinthClient,
	injectModrinthServerContext,
	injectNotificationManager,
} from '#ui/providers'
import { commonMessages } from '#ui/utils/common-messages'

type ShowOptions = {
	serverId: string
	tabIndex?: number
	tabId?: ServerSettingsTabId
}

const props = defineProps<{
	siteUrl?: string
	browseModpacks?: (args: { serverId: string; from: 'reset-server' }) => void | Promise<void>
}>()

const { formatMessage } = useVIntl()
const queryClient = useQueryClient()
const client = injectModrinthClient()
const { addNotification } = injectNotificationManager()

const messages = defineMessages({
	failedToLoadServer: {
		id: 'app.server-settings.failed-to-load-server',
		defaultMessage: 'Failed to load server settings',
	},
})

const modal = ref<InstanceType<typeof TabbedModal> | null>(null)

const { serverId: currentServerId, core, server } = injectModrinthServerContext()

// The Core the server runs on, named in the title.
const { data: coreInfo } = useQuery({
	queryKey: ['cores', 'of', currentServerId],
	queryFn: () => client.cores.coreOf(currentServerId),
})

const isApp = ref(true)

const serverSettingsTabComponentMap = {
	general: ServerSettingsGeneralPage,
	installation: ServerSettingsInstallationPage,
	sharing: ServerSettingsSharingPage,
	backups: ServerSettingsBackupsPage,
	properties: ServerSettingsPropertiesPage,
	advanced: ServerSettingsAdvancedPage,
} as const

const saveBannerTarget = ref<HTMLElement | null>(null)
const saveBannerShown = ref(false)

provideServerSettings({
	siteUrl: computed(() => (props.siteUrl ?? 'https://modrinth.com').replace(/\/$/, '')),
	isApp,
	browseModpacks: props.browseModpacks ?? (() => {}),
	closeModal: () => hide(),
	saveBanner: { target: saveBannerTarget, shown: saveBannerShown },
})

const tabs = computed<TabbedModalTab[]>(() =>
	serverSettingsTabDefinitions.map((tab) => {
		const ctx = {
			serverStatus: server.value?.status,
			// On a Core only a server that follows no instance has a share of its own; one that
			// follows an instance reports that instance's share, which it does not manage.
			isShared: !server.value?.linked_instance && !!server.value?.shared_instance_id,
		}
		const name = defineMessage({
			id: `server.settings.tabs.${tab.id}`,
			defaultMessage: tab.label,
		})
		return {
			name,
			icon: tab.icon,
			content: serverSettingsTabComponentMap[tab.id],
			shown: tab.shown ? tab.shown(ctx) : true,
		}
	}),
)

watch(tabs, (currentTabs, previousTabs) => {
	if (!modal.value) return
	const selectedTab = previousTabs.filter((tab) => tab.shown !== false)[modal.value.selectedTab]
	const selectedIndex = currentTabs
		.filter((tab) => tab.shown !== false)
		.findIndex((tab) => tab.content === selectedTab?.content)
	modal.value.setTab(Math.max(selectedIndex, 0))
})

function resolveVisibleTabIndex(tabIndex?: number, tabId?: ServerSettingsTabId) {
	const visibleTabs = tabs.value.filter((tab) => tab.shown !== false)
	let requestedTab = tabIndex ?? 0
	if (tabId) {
		const defIndex = serverSettingsTabDefinitions.findIndex((d) => d.id === tabId)
		const visibleIndex = visibleTabs.findIndex((tab) => tabs.value.indexOf(tab) === defIndex)
		if (visibleIndex >= 0) requestedTab = visibleIndex
	}
	return Math.min(Math.max(requestedTab, 0), Math.max(visibleTabs.length - 1, 0))
}

async function show({ serverId, tabIndex, tabId }: ShowOptions) {
	try {
		const targetServerId = currentServerId
		if (serverId !== targetServerId) {
			console.warn(
				`[ServerSettingsModal] Ignoring mismatched serverId "${serverId}" in favor of context "${targetServerId}"`,
			)
		}

		modal.value?.setTab(resolveVisibleTabIndex(tabIndex, tabId))
		modal.value?.show()
		const params = { params: { id: targetServerId } }
		void queryClient.prefetchQuery({
			queryKey: serverKeys.properties(targetServerId),
			queryFn: () => core.call(coreRoutes.GetProperties, params),
		})
		void queryClient.prefetchQuery({
			queryKey: serverKeys.startup(targetServerId),
			queryFn: () => core.call(coreRoutes.GetStartup, params),
		})
	} catch (error) {
		console.error(error)
		addNotification({
			type: 'error',
			title: formatMessage(messages.failedToLoadServer),
		})
	}
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
		:floating-action-bar-shown="saveBannerShown"
		:max-width="'min(980px, calc(95vw - 2rem))'"
		:width="'min(980px, calc(95vw - 2rem))'"
	>
		<template #floating-action-bar>
			<div ref="saveBannerTarget" />
		</template>
		<template #title>
			<span class="flex items-center gap-2 text-lg font-semibold text-primary">
				<template v-if="coreInfo">{{ coreInfo.name }} <ChevronRightIcon /></template>
				{{ server?.name || 'Server' }} <ChevronRightIcon />
				<span class="font-extrabold text-contrast">{{
					formatMessage(commonMessages.settingsLabel)
				}}</span>
			</span>
		</template>
	</TabbedModal>
</template>
