import { computed, type Ref, ref } from 'vue'

import { useSocial } from '@/composables/useSocial'

import type { SyncedPermissionPreset } from './use-synced-permissions'

/**
 * Resolves the current user's effective permission preset for a synced profile
 * from real friend-group membership data, with a manual override for the
 * "View as" preview affordance.
 *
 * A synced profile's server side is hosted on a Core instance. When the user
 * belongs to a friend group bound to that Core, their `role`/`permissionPreset`
 * decides what they may do. A solo user (no linked group) keeps full owner
 * access so the local-only experience is never gated.
 */

const VALID_PRESETS: SyncedPermissionPreset[] = [
	'owner',
	'admin',
	'member',
	'client-only',
	'viewer',
]

function normalizePreset(value: string | undefined): SyncedPermissionPreset | null {
	return value && (VALID_PRESETS as string[]).includes(value)
		? (value as SyncedPermissionPreset)
		: null
}

function roleToPreset(role: 'owner' | 'admin' | 'member'): SyncedPermissionPreset {
	return role
}

export interface SyncedRolePreset {
	/** Writable preset ref — assigning to it sets a manual "View as" override. */
	preset: Ref<SyncedPermissionPreset>
	/** Whether a friend group governs this profile's permissions. */
	isLinked: Ref<boolean>
	/** Drop any manual override and follow real membership data again. */
	resetOverride: () => void
}

export function useSyncedRolePreset(_serverInstanceId: string): SyncedRolePreset {
	const { group } = useSocial()
	const manualPreset = ref<SyncedPermissionPreset | null>(null)

	// The desktop app drives a single local Core, so the user's primary friend
	// group (the one bound to that Core) governs synced permissions regardless of
	// which instance the server side points at.
	const linkedGroup = computed(() => group.value)

	const isLinked = computed(() => linkedGroup.value !== null)

	const derivedPreset = computed<SyncedPermissionPreset>(() => {
		const g = linkedGroup.value
		if (!g) return 'owner'
		return normalizePreset(g.permissionPreset) ?? roleToPreset(g.role)
	})

	const preset = computed<SyncedPermissionPreset>({
		get: () => manualPreset.value ?? derivedPreset.value,
		set: (value) => {
			manualPreset.value = value
		},
	})

	return {
		preset,
		isLinked,
		resetOverride: () => {
			manualPreset.value = null
		},
	}
}
