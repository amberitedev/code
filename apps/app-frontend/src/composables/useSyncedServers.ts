/**
 * useSyncedServers — reactive access to a friend group's synced server profiles.
 *
 * Wraps the Convex `sync:*` functions through the shared social client. Owns the
 * profile list for one group plus the actions to change a profile's visibility
 * (private/selective servers) and its automatic-whitelist configuration. Errors
 * are captured into `error` so calls never escape into Tauri's uncaught handler.
 */
import type {
	ConvexSyncedProfile,
	ProfileWhitelistResult,
	SyncedProfileSettings,
} from '@amberite/amberite-api'
import type { Ref } from 'vue'
import { ref } from 'vue'

import { useSocialClient } from '@/composables/useSocialClient'

export interface UseSyncedServersReturn {
	profiles: Ref<ConvexSyncedProfile[]>
	loading: Ref<boolean>
	error: Ref<Error | null>
	refresh: (friendGroupId: string) => Promise<void>
	updateSettings: (profileId: string, settings: SyncedProfileSettings) => Promise<void>
	getWhitelist: (profileId: string) => Promise<ProfileWhitelistResult | null>
}

export function useSyncedServers(): UseSyncedServersReturn {
	const client = useSocialClient()
	const profiles = ref<ConvexSyncedProfile[]>([])
	const loading = ref(false)
	const error = ref<Error | null>(null)
	let lastGroupId: string | null = null

	async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
		loading.value = true
		error.value = null
		try {
			return await fn()
		} catch (e) {
			error.value = e instanceof Error ? e : new Error(String(e))
			return undefined
		} finally {
			loading.value = false
		}
	}

	async function refresh(friendGroupId: string): Promise<void> {
		lastGroupId = friendGroupId
		await run(async () => {
			profiles.value = await client.listServerProfiles(friendGroupId)
		})
	}

	async function updateSettings(profileId: string, settings: SyncedProfileSettings): Promise<void> {
		await run(() => client.updateSyncedProfileSettings(profileId, settings))
		if (lastGroupId) await refresh(lastGroupId)
	}

	async function getWhitelist(profileId: string): Promise<ProfileWhitelistResult | null> {
		return (await run(() => client.getProfileWhitelist(profileId))) ?? null
	}

	return { profiles, loading, error, refresh, updateSettings, getWhitelist }
}
