<script setup lang="ts">
import { DashboardIcon, ServerStackIcon, SettingsIcon, UnlinkIcon } from '@modrinth/assets'
import {
	ButtonStyled,
	EmptyState,
	injectNotificationManager,
	NavTabs,
} from '@modrinth/ui'
import { computed, ref, watch } from 'vue'

import CoreAccessPanel from '@/components/core/CoreAccessPanel.vue'
import CoreActivityPanel from '@/components/core/CoreActivityPanel.vue'
import CoreOnboardingModal from '@/components/core/CoreOnboardingModal.vue'
import CoreOverviewPanel from '@/components/core/CoreOverviewPanel.vue'
import CoreSetupPanel from '@/components/core/CoreSetupPanel.vue'
import CoreHostingSettingsModal from '@/components/core/settings/CoreHostingSettingsModal.vue'
import { useCoreClient } from '@/composables/useCoreClient'
import { useCoreConnection } from '@/composables/useCoreConnection'
import { useSocial } from '@/composables/useSocial'
import { useSocialClientRaw } from '@/composables/useSocialClient'
import { clearConnectedCore, useConnectedCore } from '@/core/connected-core'

const social = useSocial()
const coreClient = useCoreClient()
const socialClient = useSocialClientRaw()
const connection = useCoreConnection()
const connectedCore = useConnectedCore()
const { addNotification } = injectNotificationManager()
const onboardingModal = ref<InstanceType<typeof CoreOnboardingModal>>()
const settingsModal = ref<InstanceType<typeof CoreHostingSettingsModal>>()
const activeTab = ref<'overview' | 'members' | 'activity'>('overview')
const clearingLinkedCore = ref(false)
const hasConnectedCore = computed(() => !!connectedCore.value)
const onboardingVisible = ref(false)
const setupVisible = ref(!hasConnectedCore.value)

watch(hasConnectedCore, (hasCore) => {
	if (hasCore && onboardingVisible.value) return
	setupVisible.value = !hasCore
})
const tabs = [
	{ label: 'Overview', href: 'overview' },
	{ label: 'Members', href: 'members' },
	{ label: 'Activity', href: 'activity' },
]
const activeTabIndex = computed(() => tabs.findIndex((tab) => tab.href === activeTab.value))
const statusState = computed(() => connection.status.value?.state)
const statusLabel = computed(() => {
	if (!connectedCore.value) return 'No Core linked'
	if (connection.loading.value) return 'Checking'
	if (!statusState.value) return 'Unknown'
	return statusState.value === 'connected' ? 'Online' : 'Offline'
})
const statusClass = computed(() => {
	if (!connectedCore.value) return 'bg-red'
	if (connection.loading.value || !statusState.value) return 'bg-orange'
	return statusState.value === 'connected' ? 'bg-green' : 'bg-red'
})
const statusTooltip = computed(() => {
	if (!connectedCore.value) return 'No Core is linked to this app.'
	return `Core ${connectedCore.value.coreId}: ${statusLabel.value.toLowerCase()}`
})
const clearLinkedCoreLabel = computed(() =>
	clearingLinkedCore.value ? 'Clearing linked Core' : 'Clear linked Core',
)
const devViewToggleLabel = computed(() => (setupVisible.value ? 'Show dashboard' : 'Show setup'))
const devViewToggleTooltip = computed(() =>
	setupVisible.value ? 'Show Core dashboard' : 'Show Core setup',
)
const devViewToggleIcon = computed(() => (setupVisible.value ? DashboardIcon : ServerStackIcon))

function selectTab(tab: { href: string }) {
	if (tab.href === 'overview' || tab.href === 'members' || tab.href === 'activity') {
		activeTab.value = tab.href
	}
}

function toggleCoreViewForDev() {
	setupVisible.value = !setupVisible.value
}

function showOnboarding(flow: 'create' | 'connect') {
	if (connectedCore.value) return
	onboardingVisible.value = true
	onboardingModal.value?.show(flow)
}

function handleOnboardingHide() {
	onboardingVisible.value = false
}

function handleOnboardingFinish() {
	onboardingVisible.value = false
	setupVisible.value = false
	activeTab.value = 'overview'
}

function openPermissionsSettings() {
	settingsModal.value?.show('members')
}

function openGeneralSettings() {
	settingsModal.value?.show('general')
}

function clearErrorMessage(reason: unknown): string {
	return reason instanceof Error ? reason.message : String(reason)
}

async function clearLinkedCoreForDev() {
	if (clearingLinkedCore.value) return
	clearingLinkedCore.value = true
	const coreId = connectedCore.value?.coreId
	const coreUrl = connectedCore.value?.url

	try {
		await socialClient.rawMutation<{ clearedCoreIds: string[] }>(
			'dev:clearCoreLink',
			coreId ? { coreId } : {},
		)
		if (coreUrl) await coreClient.devResetSetupAt(coreUrl)
		clearConnectedCore()
		coreClient.clearCoreUrlCache()
		setupVisible.value = true
		await social.refresh()
		await connection.check()
	} catch (reason) {
		addNotification({
			type: 'error',
			title: 'Failed to clear linked Core',
			text: clearErrorMessage(reason),
		})
	} finally {
		clearingLinkedCore.value = false
	}
}
</script>

<template>
	<div class="flex min-h-full flex-col p-6">
		<CoreOnboardingModal
			ref="onboardingModal"
			@hide="handleOnboardingHide"
			@finish="handleOnboardingFinish"
		/>
		<CoreHostingSettingsModal ref="settingsModal" />
		<CoreSetupPanel
			v-if="setupVisible"
			@create="showOnboarding('create')"
			@connect="showOnboarding('connect')"
		/>
		<div v-else class="flex w-full flex-1 flex-col gap-4">
			<div class="flex items-start justify-between gap-4">
				<div class="flex min-w-0 flex-wrap items-center gap-3">
					<NavTabs
						mode="local"
						:links="tabs"
						:active-index="activeTabIndex"
						@tab-click="(_index, tab) => selectTab(tab)"
					/>
				</div>
				<div class="flex shrink-0 items-center gap-2">
					<div
						v-tooltip="statusTooltip"
						class="flex h-10 items-center gap-2 text-sm font-semibold text-secondary transition-colors hover:text-contrast"
					>
						<span
							class="size-3 rounded-full"
							:class="[statusClass, connection.loading.value ? 'animate-pulse' : '']"
							aria-hidden="true"
						/>
						<span>{{ statusLabel }}</span>
					</div>
					<ButtonStyled circular>
						<button
							v-tooltip="'Settings'"
							class="!h-12 !w-12"
							aria-label="Settings"
							@click="settingsModal?.show()"
						>
							<SettingsIcon class="!h-6 !w-6" />
						</button>
					</ButtonStyled>
				</div>
			</div>
			<EmptyState
				v-if="!hasConnectedCore"
				type="empty"
				heading="No friend group linked"
				description="Create or connect a Core to show its friend group profile."
			/>
			<CoreOverviewPanel
				v-else-if="activeTab === 'overview'"
				@open-settings="openGeneralSettings"
			/>
			<CoreAccessPanel
				v-else-if="activeTab === 'members'"
				@open-permissions-settings="openPermissionsSettings"
			/>
			<CoreActivityPanel v-else />
		</div>
		<Teleport to="body">
			<div class="fixed z-20 flex items-center gap-2" style="right: 1.25rem; bottom: 1.25rem">
				<ButtonStyled>
					<button
						v-tooltip="devViewToggleTooltip"
						class="!h-10"
						@click="toggleCoreViewForDev"
					>
						<component :is="devViewToggleIcon" />
						{{ devViewToggleLabel }}
					</button>
				</ButtonStyled>
				<ButtonStyled v-if="connectedCore" color="orange">
					<button
						v-tooltip="
							'Dev reset: clear this linked Core and make Core generate a new pairing code'
						"
						class="!h-10"
						:disabled="clearingLinkedCore"
						@click="clearLinkedCoreForDev"
					>
						<UnlinkIcon />
						{{ clearLinkedCoreLabel }}
					</button>
				</ButtonStyled>
			</div>
		</Teleport>
	</div>
</template>
