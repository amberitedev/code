import type { ConnectionStatus } from '@amberite/amberite-api'
import type { Ref } from 'vue'
import { onBeforeUnmount, ref } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'

export interface CoreConnectionState {
	status: Ref<ConnectionStatus | null>
	loading: Ref<boolean>
	error: Ref<Error | null>
	check: () => Promise<ConnectionStatus | null>
}

export function useCoreConnection(): CoreConnectionState {
	const client = useCoreClient()
	const status = ref<ConnectionStatus | null>(client.monitor?.currentStatus ?? null)
	const loading = ref(false)
	const error = ref<Error | null>(null)

	const unsubscribe = client.monitor?.onStatus((next) => {
		status.value = next
	})

	onBeforeUnmount(() => unsubscribe?.())

	async function check(): Promise<ConnectionStatus | null> {
		loading.value = true
		error.value = null

		try {
			const next = await client.connect()
			status.value = next
			return next
		} catch (e) {
			error.value = e instanceof Error ? e : new Error(String(e))
			return null
		} finally {
			loading.value = false
		}
	}

	return { status, loading, error, check }
}
