import { type InstallJobSnapshot, installs } from '@amberite/contracts/engine'
import type { ContractClient } from '@modrinth/api-client'
import { computed, type MaybeRefOrGetter, ref, toValue, watch } from 'vue'

export interface InstallJobsOptions {
	/** `client.local` for an instance on this computer, `client.core(id)` for a server. */
	client: MaybeRefOrGetter<ContractClient>
	instanceId: MaybeRefOrGetter<string>
	/** Listens to the host's `install_job` events until the returned function is called. */
	onJob: (handler: (job: InstallJobSnapshot) => void) => () => void
}

function unfinished(job: InstallJobSnapshot) {
	return job.status === 'queued' || job.status === 'running'
}

/**
 * An instance's install jobs on its host, from the engine's job list and its `install_job`
 * events, which an App and a Core both send: the Install Progress provider for either. `active`
 * is the job running or waiting now, `failed` the last one that failed.
 */
export function useInstallJobs(options: InstallJobsOptions) {
	const jobs = ref(new Map<string, InstallJobSnapshot>())
	const mine = (job: InstallJobSnapshot) => job.instance_id === toValue(options.instanceId)

	function record(job: InstallJobSnapshot) {
		if (!mine(job)) return
		const next = new Map(jobs.value)
		next.set(job.job_id, job)
		jobs.value = next
	}

	/** Reads the unfinished jobs again, as after a resync. */
	async function refresh() {
		const listed = await toValue(options.client)
			.call(installs.ListInstallJobs, { query: { include_finished: false } })
			.catch(() => [])
		const next = new Map([...jobs.value].filter(([, job]) => !unfinished(job)))
		for (const job of listed) if (mine(job)) next.set(job.job_id, job)
		jobs.value = next
	}

	watch(
		() => toValue(options.instanceId),
		() => {
			jobs.value = new Map()
			void refresh()
		},
		{ immediate: true },
	)
	const stop = options.onJob(record)

	const sorted = computed(() =>
		[...jobs.value.values()].sort((a, b) => a.created.localeCompare(b.created)),
	)
	return {
		jobs: sorted,
		active: computed(() => sorted.value.findLast(unfinished) ?? null),
		/** The last job that ended, when it failed. */
		failed: computed(() => {
			const last = sorted.value.findLast((job) => !unfinished(job))
			return last?.status === 'failed' || last?.status === 'interrupted' ? last : null
		}),
		refresh,
		stop,
	}
}
