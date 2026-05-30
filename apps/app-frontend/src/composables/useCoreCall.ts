/**
 * useCoreCall — safety wrapper for every Amberite Core API call.
 *
 * Ensures that no call can throw uncaught into Tauri's runtime. All failures are
 * caught and exposed through a reactive `error` ref. Endpoint timeout/retry policy
 * lives in @amberite/amberite-api; this composable owns Vue state and error strategy.
 *
 * Usage:
 *   const { data, loading, error, execute } = useCoreCall((client) => client.listInstances())
 *   await execute() // never throws — check error.value afterward
 *
 *   // With options:
 *   const { execute } = useCoreCall((client) => client.start(id), {
 *     timeout: 15_000,
 *     retry: 2,
 *     onError: (e) => console.warn('start failed', e),
 *   })
 */
import type { CommunicationPolicyOverride, CoreApiClient } from '@amberite/amberite-api'
import type { Ref } from 'vue'
import { ref } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'

export interface CoreCallOptions extends CommunicationPolicyOverride {
	/** Alias for CommunicationPolicy.timeoutMs. */
	timeout?: number
	/** Alias for CommunicationPolicy.retries. */
	retry?: number
	/**
	 * What to do when the call fails after all retries.
	 * - 'silent' (default): error goes into the ref, nothing thrown.
	 * - 'throw': re-throws from execute().
	 * - function: called with the error, then silently returns null.
	 */
	onError?: 'silent' | 'throw' | ((error: unknown) => void)
}

export interface CoreCallResult<T> {
	data: Ref<T | null>
	loading: Ref<boolean>
	error: Ref<Error | null>
	execute: () => Promise<T | null>
}

export function useCoreCall<T>(
	fn: (client: CoreApiClient) => Promise<T>,
	options: CoreCallOptions = {},
): CoreCallResult<T> {
	const client = useCoreClient()
	const data = ref<T | null>(null) as Ref<T | null>
	const loading = ref(false)
	const error = ref<Error | null>(null)

	async function execute(): Promise<T | null> {
		loading.value = true
		error.value = null

		try {
			const { onError: _onError, timeout, retry, ...policy } = options
			const result = await fn(
				client.withPolicy({
					...policy,
					timeoutMs: policy.timeoutMs ?? timeout,
					retries: policy.retries ?? retry,
				}),
			)
			data.value = result
			loading.value = false
			return result
		} catch (e) {
			const lastError = e instanceof Error ? e : new Error(String(e))
			error.value = lastError
			loading.value = false

			const strategy = options.onError ?? 'silent'
			if (strategy === 'throw') throw lastError
			if (typeof strategy === 'function') strategy(lastError)
			return null
		}
	}

	return { data, loading, error, execute }
}
