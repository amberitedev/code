<script setup lang="ts">
import { LinkIcon, ServerStackIcon, SettingsIcon } from '@modrinth/assets'
import { ButtonStyled, NavTabs, Toggle, useLoadingBarToken } from '@modrinth/ui'
import { computed, onUnmounted, ref, watch } from 'vue'

import AppPageSkeleton from '@/components/ui/AppPageSkeleton.vue'
import CoreAccessPanel from '@/components/core/CoreAccessPanel.vue'
import CoreActivityPanel from '@/components/core/CoreActivityPanel.vue'
import CoreOnboardingModal from '@/components/core/CoreOnboardingModal.vue'
import CoreSetupPanel from '@/components/core/CoreSetupPanel.vue'
import CoreHostingSettingsModal from '@/components/core/settings/CoreHostingSettingsModal.vue'
import { useOptimisticLoading } from '@/composables/useOptimisticPreload'
import { useCoreConnection } from '@/composables/useCoreConnection'
import { useSocial } from '@/composables/useSocial'
import { useConnectedCore } from '@/core/connected-core'

const social = useSocial()
const connection = useCoreConnection()
const connectedCore = useConnectedCore()
const onboardingModal = ref<InstanceType<typeof CoreOnboardingModal>>()
const settingsModal = ref<InstanceType<typeof CoreHostingSettingsModal>>()
const activeTab = ref<'overview' | 'activity'>('overview')
const hasResolvedDashboard = ref(!social.loading.value)
watch(
	() => social.loading.value,
	(loading) => {
		if (!loading) hasResolvedDashboard.value = true
	},
	{ immediate: true },
)
const dashboardPending = computed(() => social.loading.value && !hasResolvedDashboard.value)
const hasDashboardDecision = computed(() => hasResolvedDashboard.value)
const initialDashboardSkeleton = useOptimisticLoading(dashboardPending, hasDashboardDecision)
const forceDashboardSkeleton = ref(false)
let forceDashboardSkeletonTimeout: ReturnType<typeof window.setTimeout> | null = null
const showDashboardSkeleton = computed(
	() => initialDashboardSkeleton.value || forceDashboardSkeleton.value,
)
useLoadingBarToken(dashboardPending)
const hasGroup = computed(() => !!social.group.value)
const setupVisible = ref(!hasGroup.value || !connectedCore.value)

watch([hasGroup, connectedCore], ([has, core], [hadGroup, previousCore]) => {
	if (!has || !core) setupVisible.value = true
	else if (!hadGroup || !previousCore) setupVisible.value = false
})
const tabs = [
	{ label: 'Overview', href: 'overview' },
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

function selectTab(tab: { href: string }) {
	if (tab.href === 'overview' || tab.href === 'activity') {
		activeTab.value = tab.href
	}
}

function setForceDashboardSkeleton(enabled: boolean) {
	if (forceDashboardSkeletonTimeout !== null) {
		window.clearTimeout(forceDashboardSkeletonTimeout)
		forceDashboardSkeletonTimeout = null
	}

	forceDashboardSkeleton.value = enabled

	if (!enabled) return

	forceDashboardSkeletonTimeout = window.setTimeout(() => {
		forceDashboardSkeleton.value = false
		forceDashboardSkeletonTimeout = null
	}, 3000)
}

onUnmounted(() => {
	if (forceDashboardSkeletonTimeout !== null) {
		window.clearTimeout(forceDashboardSkeletonTimeout)
	}
})
</script>

<template>
	<div class="flex min-h-full flex-col p-6">
		<CoreOnboardingModal ref="onboardingModal" />
		<CoreHostingSettingsModal ref="settingsModal" />
		<AppPageSkeleton v-if="showDashboardSkeleton" variant="core-overview" class="!p-0" />
		<CoreSetupPanel
			v-else-if="setupVisible"
			@create="onboardingModal?.show('create')"
			@connect="onboardingModal?.show('connect')"
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
			<CoreAccessPanel v-if="activeTab === 'overview'" />
			<CoreActivityPanel v-else />
		</div>
		<Teleport to="body">
			<div
				class="fixed z-20 flex items-center gap-2"
				style="right: 1.25rem; bottom: 1.25rem"
			>
				<div class="rounded-full border border-solid border-surface-5 bg-surface-3 p-2 shadow-lg">
					<Toggle
						id="core-force-ghost-toggle"
						:model-value="forceDashboardSkeleton"
						v-tooltip="'Force dashboard ghost for 3 seconds'"
						small
						aria-label="Force dashboard ghost for 3 seconds"
						@update:model-value="setForceDashboardSkeleton"
					/>
				</div>
				<ButtonStyled v-if="!showDashboardSkeleton" circular>
					<button
						v-tooltip="setupVisible ? 'Show dashboard' : 'Show setup'"
						class="!h-10 !w-10"
						:aria-label="setupVisible ? 'Show dashboard' : 'Show setup'"
						@click="setupVisible = !setupVisible"
					>
						<component :is="setupVisible ? ServerStackIcon : LinkIcon" />
					</button>
				</ButtonStyled>
			</div>
		</Teleport>
	</div>
</template>
