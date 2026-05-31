<template>
	<NewModal
		ref="modal"
		:header="undefined"
		hide-header
		max-width="60rem"
		@hide="controller.close()"
	>
		<div class="flex min-h-[32rem] w-full">
			<nav
				class="flex w-48 shrink-0 flex-col gap-1 border-0 border-r border-solid border-divider p-3"
			>
				<h2 class="m-0 mb-2 px-2 text-lg font-extrabold text-contrast">Server settings</h2>
				<button
					v-for="tab in visibleTabs"
					:key="tab.id"
					type="button"
					class="flex items-center gap-2 rounded-xl border-none bg-transparent px-3 py-2 text-left text-sm font-bold text-secondary transition-colors hover:bg-button-bg hover:text-contrast"
					:class="{ 'bg-button-bg !text-contrast': tab.id === controller.activeTab.value }"
					@click="controller.activeTab.value = tab.id"
				>
					<component :is="tab.icon" class="h-4 w-4" />
					{{ tab.label }}
				</button>
			</nav>
			<div class="min-w-0 flex-1 overflow-y-auto p-5">
				<Suspense>
					<component :is="activeComponent" :key="controller.activeTab.value" />
					<template #fallback>
						<div class="flex h-full items-center justify-center">
							<SpinnerIcon class="h-6 w-6 animate-spin text-secondary" />
						</div>
					</template>
				</Suspense>
			</div>
		</div>
	</NewModal>
</template>

<script setup lang="ts">
import { SpinnerIcon } from '@modrinth/assets'
import { NewModal } from '@modrinth/ui'
import { computed, ref, watch } from 'vue'

import { injectCoreServerContext } from '../core-server-instance'
import { injectServerSettingsController } from './server-settings-controller'
import { serverSettingsTabs } from './tabs'

const controller = injectServerSettingsController()
const { server } = injectCoreServerContext()

const modal = ref<{ show: () => void; hide: () => void } | null>(null)

const visibleTabs = computed(() =>
	serverSettingsTabs.filter((tab) => !tab.shown || tab.shown(server.value?.status ?? 'available')),
)

const activeComponent = computed(
	() =>
		visibleTabs.value.find((tab) => tab.id === controller.activeTab.value)?.component ??
		visibleTabs.value[0]?.component,
)

watch(
	() => controller.isOpen.value,
	(open) => {
		if (open) modal.value?.show()
		else modal.value?.hide()
	},
)
</script>
