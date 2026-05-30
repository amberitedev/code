/**
 * useCoreInstances — reactive instance list backed by SSE push events.
 *
 * Fetches the initial list via CoreApiClient.listInstances(), then opens a
 * CoreEventStream and patches the reactive Map in-place as events arrive.
 * The SSE connection auto-reconnects with a 3-second delay on close or error.
 *
 * Usage:
 *   const { instances, loading, error, refresh, start, stop, restart, kill } =
 *     useCoreInstances()
 *
 * Each action (start/stop/restart/kill) accepts a CoreCallOptions override so
 * the caller controls timeout, retry, and error strategy per call:
 *   await start(id, { timeout: 20_000, onError: (e) => toast.error(e.message) })
 */
import type { CoreEventStream, CoreInstanceSummary } from '@amberite/amberite-api'
import type { Ref } from 'vue'
import { onMounted, onUnmounted, ref } from 'vue'

import type { CoreCallOptions } from '@/composables/useCoreCall'
import { useCoreCall } from '@/composables/useCoreCall'
import { useCoreClient } from '@/composables/useCoreClient'

export interface UseCoreInstancesReturn {
	/** Reactive map of instance id → summary. Patched live by SSE. */
	instances: Ref<Map<string, CoreInstanceSummary>>
	loading: Ref<boolean>
	error: Ref<Error | null>
	/** Re-fetches the full list from Core and replaces the map. */
	refresh: () => Promise<void>
	start: (id: string, opts?: CoreCallOptions) => Promise<void>
	stop: (id: string, opts?: CoreCallOptions) => Promise<void>
	restart: (id: string, opts?: CoreCallOptions) => Promise<void>
	kill: (id: string, opts?: CoreCallOptions) => Promise<void>
}

export function useCoreInstances(): UseCoreInstancesReturn {
	const client = useCoreClient()
	const instances = ref<Map<string, CoreInstanceSummary>>(new Map())

	const { loading, error, execute: fetchList } = useCoreCall((core) => core.listInstances())

	let stream: CoreEventStream | null = null
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null
	let unmounted = false

	async function refresh(): Promise<void> {
		const list = await fetchList()
		if (list) {
			const map = new Map<string, CoreInstanceSummary>()
			for (const inst of list) map.set(inst.id, inst)
			instances.value = map
		}
	}

	function patchInstance(id: string, patch: Partial<CoreInstanceSummary>): void {
		const existing = instances.value.get(id)
		if (!existing) return
		instances.value = new Map(instances.value).set(id, { ...existing, ...patch })
	}

	async function openStream(): Promise<void> {
		if (unmounted) return
		try {
			stream = await client.openEvents()

			stream.onEvent((event) => {
				if (event.type === 'instance_created' || event.type === 'instance_updated') {
					instances.value = new Map(instances.value).set(event.instance.id, event.instance)
				} else if (event.type === 'instance_deleted') {
					const map = new Map(instances.value)
					map.delete(event.instance_id)
					instances.value = map
				} else if (event.type === 'status_changed') {
					patchInstance(event.instance_id, { status: event.status })
				} else if (event.type === 'install_status_changed') {
					patchInstance(event.instance_id, { install_status: event.install_status })
				}
			})

			stream.onError(() => {
				if (!unmounted) scheduleReconnect()
			})

			stream.onClose(() => {
				if (!unmounted) scheduleReconnect()
			})
		} catch {
			if (!unmounted) scheduleReconnect()
		}
	}

	function scheduleReconnect(): void {
		if (reconnectTimer !== null || unmounted) return
		reconnectTimer = setTimeout(() => {
			reconnectTimer = null
			void openStream()
		}, 3_000)
	}

	onMounted(async () => {
		await refresh()
		void openStream()
	})

	onUnmounted(() => {
		unmounted = true
		if (reconnectTimer !== null) {
			clearTimeout(reconnectTimer)
			reconnectTimer = null
		}
		stream?.close()
		stream = null
	})

	function makeAction(fn: (core: typeof client, id: string) => Promise<void>) {
		return (id: string, opts?: CoreCallOptions): Promise<void> => {
			const { execute } = useCoreCall((core) => fn(core, id), opts)
			return execute().then(() => undefined)
		}
	}

	return {
		instances,
		loading,
		error,
		refresh,
		start: makeAction((core, id) => core.start(id)),
		stop: makeAction((core, id) => core.stop(id)),
		restart: makeAction((core, id) => core.restart(id)),
		kill: makeAction((core, id) => core.kill(id)),
	}
}
