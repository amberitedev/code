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
import { onUnmounted, ref } from 'vue'
import { makeFunctionReference } from 'convex/server'

import { useRealtimeConvexClient, useSocialClient } from '@/composables/useSocialClient'

const serverProfilesState = makeFunctionReference<'query', { friendGroupId: string }, ConvexSyncedProfile[]>('sync:serverProfilesState')

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
	let unsubscribe: (() => void) | null = null
	let subscribedFriendGroupId: string | null = null

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
		if (subscribedFriendGroupId === friendGroupId && unsubscribe) return
		unsubscribe?.()
		unsubscribe = null
		subscribedFriendGroupId = friendGroupId
		loading.value = true
		unsubscribe = useRealtimeConvexClient().onUpdate(serverProfilesState, { friendGroupId }, (next) => {
			profiles.value = next
			loading.value = false
			error.value = null
		}, (reason) => {
			loading.value = false
			error.value = reason
		})
	}

	onUnmounted(() => {
		unsubscribe?.()
		unsubscribe = null
		subscribedFriendGroupId = null
	})

	async function updateSettings(profileId: string, settings: SyncedProfileSettings): Promise<void> {
		await run(() => client.updateSyncedProfileSettings(profileId, settings))
	}

	async function getWhitelist(profileId: string): Promise<ProfileWhitelistResult | null> {
		return (await run(() => client.getProfileWhitelist(profileId))) ?? null
	}

	return { profiles, loading, error, refresh, updateSettings, getWhitelist }
}
