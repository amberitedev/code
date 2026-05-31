import type { InjectionKey, Ref } from 'vue'
import { inject, ref } from 'vue'

import type { ServerSettingsTabId } from './tabs'

/**
 * Reactive controller for the forked Core server-settings modal. Created once per
 * server runtime (provideCoreServerRuntime) and injected by ServerSettingsModal.vue.
 * `open(tabId?)` is what `provideServerSettingsModal.openServerSettings` delegates to.
 */
export interface ServerSettingsController {
	isOpen: Ref<boolean>
	activeTab: Ref<ServerSettingsTabId>
	open: (tabId?: ServerSettingsTabId) => void
	close: () => void
}

export const serverSettingsControllerKey: InjectionKey<ServerSettingsController> = Symbol(
	'core-server-settings-controller',
)

export function createServerSettingsController(): ServerSettingsController {
	const isOpen = ref(false)
	const activeTab = ref<ServerSettingsTabId>('general')

	return {
		isOpen,
		activeTab,
		open(tabId?: ServerSettingsTabId) {
			if (tabId) activeTab.value = tabId
			isOpen.value = true
		},
		close() {
			isOpen.value = false
		},
	}
}

export function injectServerSettingsController(): ServerSettingsController {
	const controller = inject(serverSettingsControllerKey)
	if (!controller) {
		throw new Error('ServerSettingsController not provided — call provideCoreServerRuntime first')
	}
	return controller
}
