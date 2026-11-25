import { library } from '@amberite/contracts/app'

import { apiClient } from '@/platform/modrinth-client'

export const hiddenInstanceKeys = {
	all: ['hidden-instances'] as const,
	ids: (userId: string | null) => [...hiddenInstanceKeys.all, userId] as const,
}

export const hiddenInstanceIdsQueryOptions = (userId: string | null) => ({
	networkMode: 'always' as const,
	queryKey: hiddenInstanceKeys.ids(userId),
	queryFn: () => apiClient.local.call(library.listHiddenInstances),
	staleTime: Infinity,
})
