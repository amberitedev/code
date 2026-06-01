import {
	computed,
	type ComputedRef,
	inject,
	type InjectionKey,
	provide,
	type Ref,
	ref,
	watchEffect,
} from 'vue'

import { SYNCED_SIDE_TABS, type SyncedSide, useSyncedSide } from './use-synced-side'

/**
 * Granular capabilities a user may hold on a synced profile. The UI either grays
 * out an action ("you can't do this") or hides a whole section based on these.
 *
 * The `<side>:view` permissions decide whether the Server / Client tab is shown
 * at all; the rest gate individual actions within a side.
 */
export type SyncedPermission =
	| 'server:view'
	| 'server:power'
	| 'server:content'
	| 'server:files'
	| 'server:backups'
	| 'server:settings'
	| 'client:view'
	| 'client:content'
	| 'client:settings'
	| 'instance:settings'
	| 'members:manage'

/**
 * Named permission bundles. These line up with `friendGroupMembers.permissionPreset`
 * in Convex, so a member's stored preset maps straight onto a permission set.
 */
export type SyncedPermissionPreset = 'owner' | 'admin' | 'member' | 'client-only' | 'viewer'

const ALL_PERMISSIONS: SyncedPermission[] = [
	'server:view',
	'server:power',
	'server:content',
	'server:files',
	'server:backups',
	'server:settings',
	'client:view',
	'client:content',
	'client:settings',
	'instance:settings',
	'members:manage',
]

export const SYNCED_PERMISSION_PRESETS: Record<SyncedPermissionPreset, SyncedPermission[]> = {
	owner: ALL_PERMISSIONS,
	admin: ALL_PERMISSIONS,
	member: ['server:view', 'client:view', 'client:content', 'client:settings', 'instance:settings'],
	'client-only': ['client:view', 'client:content', 'client:settings'],
	viewer: ['server:view', 'client:view'],
}

export const SYNCED_PERMISSION_PRESET_LABELS: Record<SyncedPermissionPreset, string> = {
	owner: 'Owner',
	admin: 'Admin',
	member: 'Member',
	'client-only': 'Client only',
	viewer: 'Viewer',
}

const syncedPermissionsKey: InjectionKey<Ref<SyncedPermissionPreset>> = Symbol('synced-permissions')

export interface SyncedPermissionsApi {
	preset: Ref<SyncedPermissionPreset>
	permissions: ComputedRef<Set<SyncedPermission>>
	has: (permission: SyncedPermission) => boolean
}

/**
 * Provided once by the synced layout. `source` is the current user's effective
 * preset for this profile; it defaults to 'owner' so a solo user keeps full
 * access. This ref is the seam where real `friendGroupMembers.role/permissionPreset`
 * data plugs in once a synced profile is linked to a friend group.
 */
export function provideSyncedPermissions(
	source: Ref<SyncedPermissionPreset> = ref<SyncedPermissionPreset>('owner'),
): Ref<SyncedPermissionPreset> {
	provide(syncedPermissionsKey, source)
	return source
}

export function useSyncedPermissions(): SyncedPermissionsApi {
	const preset = inject(syncedPermissionsKey) ?? ref<SyncedPermissionPreset>('owner')
	const permissions = computed(() => new Set(SYNCED_PERMISSION_PRESETS[preset.value]))
	const has = (permission: SyncedPermission) => permissions.value.has(permission)
	return { preset, permissions, has }
}

/**
 * Side tabs filtered to the sides the current user may view, with the active
 * side auto-corrected when its tab becomes hidden. Used by every synced
 * sub-page so disallowed sides vanish instead of showing forbidden data.
 */
export function useSyncedSideTabs() {
	const side = useSyncedSide()
	const { has } = useSyncedPermissions()

	const tabs = computed(() =>
		SYNCED_SIDE_TABS.filter((tab) => has(`${tab.id as SyncedSide}:view` as SyncedPermission)),
	)

	watchEffect(() => {
		if (tabs.value.length === 0) return
		if (!tabs.value.some((tab) => tab.id === side.value)) {
			side.value = tabs.value[0].id
		}
	})

	return { side, tabs }
}
