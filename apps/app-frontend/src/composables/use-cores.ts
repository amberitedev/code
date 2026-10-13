import { type QueryClient, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed } from 'vue'

import { apiClient } from '@/services/api-client'

export const coresQueryKey = ['cores']

const coresQueryOptions = {
	queryKey: coresQueryKey,
	queryFn: () => apiClient.archon.cores_v1.list(),
	staleTime: 30_000,
}

// Kept from the last `useCores` call so `ensureCore`, which runs from click handlers, can read the
// same cache.
let queryClient: QueryClient | null = null

/**
 * The Cores the account may reach. `cores` has all of them, including ones it was only invited
 * to manage a server on; `ownCores` has the ones signed in to the account, which is where it can
 * create servers.
 */
export function useCores() {
	queryClient = useQueryClient()
	const query = useQuery(coresQueryOptions)
	const cores = computed(() => query.data.value ?? [])
	const ownCores = computed(() => cores.value.filter((core) => core.owned))
	return { ...query, cores, ownCores }
}

// The one "Add Core" modal is mounted in App.vue and registers itself here.
let showAddCoreModal: ((code?: string) => Promise<boolean>) | null = null

export function registerAddCoreModal(show: ((code?: string) => Promise<boolean>) | null) {
	showAddCoreModal = show
}

/**
 * Open "Add Core", with the code filled in when an `amberite://approve/<code>` link brought it up.
 * Resolves true once a Core is added, false if the modal is closed.
 */
export function openAddCoreModal(code?: string): Promise<boolean> {
	return showAddCoreModal?.(code) ?? Promise.resolve(false)
}

/**
 * Call before anything that creates a server. Resolves true right away when the account has a Core
 * of its own (being invited to someone else's does not count); otherwise opens "Add Core" and
 * resolves true only once one is added. Throws when the account's Cores cannot be loaded, so a
 * failed lookup never asks for a Core. A list fetched in the last 30 seconds is reused.
 */
export async function ensureCore(): Promise<boolean> {
	const cores = await (queryClient?.fetchQuery(coresQueryOptions) ?? coresQueryOptions.queryFn())
	return cores.some((core) => core.owned) || openAddCoreModal()
}
