import { MonitorIcon, ServerIcon } from '@modrinth/assets'
import { inject, type InjectionKey, provide, type Ref, ref } from 'vue'

import type { TabViewTab } from '@/components/ui/TabView.vue'

/**
 * The two sides of a synced profile. A synced profile is a single app-lib
 * profile (drives the client side, id == route id) that is also bound 1:1 to an
 * Amberite Core instance (the server side, same id). The Server/Client TabView
 * swaps which data is shown — same interface, different source.
 */
export type SyncedSide = 'server' | 'client'

/**
 * Tab descriptors reused by every synced sub-page so the Server/Client pills look
 * identical everywhere. `color` is a token used to colour-code per-side name tags
 * (e.g. backups/worlds) so it's obvious whether an entry is server- or client-side.
 */
export const SYNCED_SIDE_TABS: TabViewTab[] = [
	{ id: 'server', label: 'Server', icon: ServerIcon, color: 'var(--color-blue)' },
	{ id: 'client', label: 'Client', icon: MonitorIcon, color: 'var(--color-green)' },
]

const syncedSideKey: InjectionKey<Ref<string>> = Symbol('synced-side')

/**
 * Provided once by the synced layout so the chosen side (Server/Client) is kept
 * consistent as you move between sub-pages (Content, Files, Backups, ...).
 */
export function provideSyncedSide(initial: SyncedSide = 'server'): Ref<string> {
	const side = ref<string>(initial)
	provide(syncedSideKey, side)
	return side
}

export function useSyncedSide(): Ref<string> {
	return inject(syncedSideKey) ?? ref<string>('server')
}

/** Opens the existing Core server settings modal (provided by the synced layout). */
export const SYNCED_OPEN_SERVER_SETTINGS: InjectionKey<() => void> = Symbol(
	'synced-open-server-settings',
)

/** Opens the existing client instance settings modal (provided by the synced layout). */
export const SYNCED_OPEN_CLIENT_SETTINGS: InjectionKey<() => void> = Symbol(
	'synced-open-client-settings',
)
