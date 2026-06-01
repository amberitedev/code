import type { ConvexModSyncEvent, ConvexProfileSnapshot } from '@amberite/amberite-api'
import { computed, ref } from 'vue'

import { useSocial } from '@/composables/useSocial'
import { useSocialClient } from '@/composables/useSocialClient'
import type { GameInstance } from '@/helpers/types'

import { buildSyncedManifest } from './synced-manifest'
import { registerSyncedProfileBackend } from './synced-registration'

/**
 * Git-style snapshot history for a synced profile.
 *
 * Resolves the backend synced-profile record (registering it on demand when a
 * friend group exists), exposes the snapshot history + mod-sync event log, and
 * publishes a new snapshot from the current content of both sides. Everything is
 * best-effort: with no friend group the feature reports "not linked" instead of
 * failing, so the local-only experience is unaffected.
 */
export function useSyncedSnapshots(
	instance: GameInstance,
	serverInstanceId: string,
	clientProfilePath: string,
) {
	const { group } = useSocial()
	const client = useSocialClient()

	const profileId = ref<string | null>(null)
	const snapshots = ref<ConvexProfileSnapshot[]>([])
	const events = ref<ConvexModSyncEvent[]>([])
	const loading = ref(false)
	const publishing = ref(false)
	const error = ref<Error | null>(null)

	const isLinked = computed(() => group.value !== null)
	const ready = computed(() => profileId.value !== null)

	async function resolveProfileId(): Promise<string | null> {
		const g = group.value
		if (!g) return null
		try {
			let profiles = await client.listServerProfiles(g.group.id)
			let match = profiles.find((p) => p.coreInstanceId === serverInstanceId)
			if (!match) {
				await registerSyncedProfileBackend({
					profilePath: clientProfilePath,
					serverInstanceId,
					instance,
				})
				profiles = await client.listServerProfiles(g.group.id)
				match = profiles.find((p) => p.coreInstanceId === serverInstanceId)
			}
			profileId.value = match?._id ?? null
			return profileId.value
		} catch (e) {
			error.value = e instanceof Error ? e : new Error(String(e))
			return null
		}
	}

	async function refresh(): Promise<void> {
		loading.value = true
		error.value = null
		try {
			const id = profileId.value ?? (await resolveProfileId())
			if (!id) {
				snapshots.value = []
				events.value = []
				return
			}
			const [snaps, evs] = await Promise.all([
				client.listProfileSnapshots(id),
				client.listModSyncEvents(id),
			])
			snapshots.value = snaps
			events.value = evs
		} catch (e) {
			error.value = e instanceof Error ? e : new Error(String(e))
		} finally {
			loading.value = false
		}
	}

	async function publish(notes?: string): Promise<void> {
		publishing.value = true
		error.value = null
		try {
			const id = profileId.value ?? (await resolveProfileId())
			if (!id) throw new Error('This synced profile is not linked to a friend group yet.')
			const manifest = await buildSyncedManifest(instance, serverInstanceId)
			await client.publishProfileSnapshot({
				profileId: id,
				manifest,
				clientOnlyManifest: { entries: manifest.client },
				serverManifest: { entries: manifest.server },
				notes: notes?.trim() || undefined,
			})
			await refresh()
		} catch (e) {
			error.value = e instanceof Error ? e : new Error(String(e))
			throw error.value
		} finally {
			publishing.value = false
		}
	}

	return { snapshots, events, loading, publishing, error, isLinked, ready, refresh, publish }
}
