import { instanceSharing } from '@amberite/contracts/engine'
import { useQuery } from '@tanstack/vue-query'
import { computed } from 'vue'

import { injectModrinthServerContext } from '#ui/providers'

import { serverKeys } from '../server-queries'
import { useSharedInstanceInstalledOnce } from './shared-instance'

/**
 * The open server's own share: its id (none while the server follows an instance), whether a
 * member installed it yet, and whether the server has changes the share's latest version lacks.
 */
export function useServerOwnShare() {
	const { serverId, core, server } = injectModrinthServerContext()
	const sharedInstanceId = computed(() =>
		server.value?.linked_instance ? null : (server.value?.shared_instance_id ?? null),
	)
	const installedOnce = useSharedInstanceInstalledOnce(sharedInstanceId)
	const preview = useQuery({
		queryKey: computed(() => serverKeys.shareDiff(serverId)),
		queryFn: () =>
			core.call(instanceSharing.GetSharedInstancePublishPreview, { params: { id: serverId } }),
		enabled: computed(() => !!sharedInstanceId.value),
	})
	return {
		sharedInstanceId,
		installedOnce,
		needsUpdate: computed(() => (preview.data.value?.diffs.length ?? 0) > 0),
	}
}
