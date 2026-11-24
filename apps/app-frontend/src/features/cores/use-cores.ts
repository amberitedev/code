import { createContext } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed } from 'vue'

import { apiClient } from '@/platform/modrinth-client'

export const coresQueryKey = ['cores']

/** The account's Cores. */
export function useCores() {
	const query = useQuery({
		queryKey: coresQueryKey,
		queryFn: () => apiClient.cores.list(),
		staleTime: 30_000,
	})
	const cores = computed(() => query.data.value ?? [])
	return { ...query, cores }
}

/**
 * Opens "Add Core" from anywhere in the app. App.vue mounts the one modal and provides these.
 *
 * `openAddCore` fills in the code when an `amberite://approve/<code>` link brought it up, and
 * resolves true once a Core is added, false if the modal is closed.
 *
 * `ensureCore` is called before anything that creates a server. It resolves true right away when
 * the account has a Core; otherwise it opens "Add Core" and resolves true only once one is added.
 * It throws when the account's Cores cannot be loaded, so a failed lookup never asks for a Core.
 */
export interface CoreActions {
	openAddCore: (code?: string) => Promise<boolean>
	ensureCore: () => Promise<boolean>
}

export const [injectCoreActions, provideCoreActions] = createContext<CoreActions>(
	'root',
	'coreActions',
)

export function createCoreActions(openAddCore: CoreActions['openAddCore']): CoreActions {
	return {
		openAddCore,
		ensureCore: async () => {
			const cores = await apiClient.cores.list()
			return cores.length > 0 || openAddCore()
		},
	}
}
