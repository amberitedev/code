import { createContext } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed } from 'vue'

import { apiClient } from '@/platform/modrinth-client'

export const coresQueryKey = ['cores']

/**
 * The Cores the account may reach. `cores` has all of them, including ones it was only invited
 * to manage a server on; `ownCores` has the ones signed in to the account, which is where it can
 * create servers.
 */
export function useCores() {
	const query = useQuery({
		queryKey: coresQueryKey,
		queryFn: () => apiClient.archon.cores_v1.list(),
		staleTime: 30_000,
	})
	const cores = computed(() => query.data.value ?? [])
	const ownCores = computed(() => cores.value.filter((core) => core.owned))
	return { ...query, cores, ownCores }
}

/**
 * Opens "Add Core" from anywhere in the app. App.vue mounts the one modal and provides these.
 *
 * `openAddCore` fills in the code when an `amberite://approve/<code>` link brought it up, and
 * resolves true once a Core is added, false if the modal is closed.
 *
 * `ensureCore` is called before anything that creates a server. It resolves true right away when
 * the account has a Core of its own (being invited to someone else's does not count); otherwise it
 * opens "Add Core" and resolves true only once one is added. It throws when the account's Cores
 * cannot be loaded, so a failed lookup never asks for a Core.
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
			const cores = await apiClient.archon.cores_v1.list()
			return cores.some((core) => core.owned) || openAddCore()
		},
	}
}
