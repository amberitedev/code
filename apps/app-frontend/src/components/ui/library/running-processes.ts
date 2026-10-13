import { useQuery, useQueryClient } from '@tanstack/vue-query'

import { get_all } from '@/helpers/process'
import { injectAppEvents } from '@/providers/app-events'

type ProcessMetadata = {
	uuid: string
	instance_id: string
	start_time: string
}

const processListKey = ['processes', 'list'] as const
let listeningForProcessEvents = false

// The running processes, shared by everything on Home. Process events keep the cached list right
// for the app's lifetime (this subscription is never removed), so a Home visit reads the list from
// cache instead of asking for it again.
export function useRunningProcesses() {
	const queryClient = useQueryClient()
	if (!listeningForProcessEvents) {
		listeningForProcessEvents = true
		injectAppEvents().on('process', () => {
			void queryClient.invalidateQueries({ queryKey: processListKey }, { cancelRefetch: false })
		})
	}

	return useQuery({
		queryKey: processListKey,
		queryFn: (): Promise<ProcessMetadata[]> => get_all(),
		staleTime: 30_000,
	})
}
