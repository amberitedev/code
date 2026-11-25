import type { InstallJobSnapshot } from '@modrinth/api-client'
import { computed, type ComputedRef, ref, watch } from 'vue'

export type ServerPlatform = 'vanilla' | 'fabric' | 'quilt' | 'forge' | 'neoforge' | 'paper'

/** What the panel says is being installed, when it started the install itself. */
export type ServerInstallationKey =
	| {
			type: 'platform'
			platform: ServerPlatform
			platform_version: string
			game_version: string
	  }
	| { type: 'modrinth_modpack'; project_id: string; version_id: string }
	| { type: 'local_modpack'; filename: string }
	| { type: 'unknown' }

export type ServerInstallationState = {
	id: string
	key: ServerInstallationKey
	status: 'pending' | 'installing' | 'complete' | 'failed'
	/** 0 to 100, or null while not known. */
	progress: number | null
	error: string | null
	/** `optimistic` until the Core's install job starts, `job` after. */
	source: 'optimistic' | 'job'
}

function percent(job: InstallJobSnapshot) {
	const progress = job.progress
	return progress && progress.total > 0
		? Math.floor((progress.current / progress.total) * 100)
		: null
}

/**
 * The installation a server shows: the install job its Core runs now, or the last one that failed
 * until it is dismissed, or the one the panel just started, before the Core's job starts.
 */
export function useServerInstallationTracker(jobs: {
	active: ComputedRef<InstallJobSnapshot | null>
	failed: ComputedRef<InstallJobSnapshot | null>
}) {
	const optimistic = ref<ServerInstallationKey | null>(null)
	const dismissed = ref(new Set<string>())
	// What the panel started ends with the job that ran it.
	watch(
		() => jobs.active.value?.job_id,
		(now, before) => {
			if (before && !now) optimistic.value = null
		},
	)

	const installation = computed<ServerInstallationState | null>(() => {
		const key = optimistic.value ?? { type: 'unknown' as const }
		const active = jobs.active.value
		if (active)
			return {
				id: active.job_id,
				key,
				status: active.status === 'queued' ? 'pending' : 'installing',
				progress: percent(active),
				error: null,
				source: 'job',
			}
		if (optimistic.value)
			return {
				id: 'optimistic',
				key,
				status: 'pending',
				progress: null,
				error: null,
				source: 'optimistic',
			}
		const failed = jobs.failed.value
		if (failed && !dismissed.value.has(failed.job_id))
			return {
				id: failed.job_id,
				key,
				status: 'failed',
				progress: null,
				error: failed.error?.message ?? failed.status,
				source: 'job',
			}
		return null
	})

	return {
		installation,
		/** Installing now, or about to. */
		isBlocking: computed(
			() => installation.value?.status === 'pending' || installation.value?.status === 'installing',
		),
		begin(key: ServerInstallationKey) {
			optimistic.value = key
		},
		cancelOptimistic() {
			optimistic.value = null
		},
		dismiss(id: string) {
			dismissed.value = new Set([...dismissed.value, id])
		},
	}
}
