<script setup lang="ts">
import { injectModrinthClient, injectNotificationManager, serverKeys } from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import ShareHistory from '@/features/share-history/share-history.vue'
import { useServerHistory } from '@/features/share-history/use-server-history'
import { getErrorMessage, install_get_shared_instance_history } from '@/helpers/install'

// A server's History tab. A server-first server's history is on its Core, which restores and
// deletes. A linked server shows its instance's history from this App, read-only; restoring and
// pushing happen on the instance.

const route = useRoute()
const client = injectModrinthClient()
const queryClient = useQueryClient()
const { addNotification } = injectNotificationManager()

const serverId = computed(() => String(route.params.id ?? ''))
const source = useServerHistory(serverId)
const historyKey = computed(() => ['server-share-history', serverId.value] as const)

const history = useQuery({
	queryKey: computed(() => [...historyKey.value, source.value?.source] as const),
	queryFn: async () => {
		const from = source.value!
		if (from.source === 'server')
			return client.archon.content_v1.getShareHistory(from.serverId, from.worldId)
		// Not on this computer, or not shared from it: nothing to show.
		return install_get_shared_instance_history(from.instanceId).catch(() => [])
	},
	enabled: () => !!source.value,
	staleTime: 0,
})

/** Runs `change` on the server's Core, then reloads what it changed. */
function useServerChange(
	change: (serverId: string, worldId: string, version: number) => Promise<void>,
	title: string,
) {
	return useMutation({
		mutationFn: (version: number) => {
			const from = source.value
			if (from?.source !== 'server') throw new Error('This server does not manage its history.')
			return change(from.serverId, from.worldId, version)
		},
		onError: (cause) => {
			console.error(title, cause)
			addNotification({ type: 'error', title, text: getErrorMessage(cause) })
		},
		onSettled: () =>
			Promise.all([
				queryClient.invalidateQueries({ queryKey: historyKey.value }),
				queryClient.invalidateQueries({ queryKey: serverKeys.content(serverId.value) }),
				queryClient.invalidateQueries({ queryKey: serverKeys.detailV1(serverId.value) }),
				queryClient.invalidateQueries({ queryKey: serverKeys.shareDiff(serverId.value) }),
			]),
	})
}
const restore = useServerChange(
	(id, world, version) => client.archon.content_v1.restoreShareVersion(id, world, version),
	'Could not restore the version',
)
const remove = useServerChange(
	(id, world, version) => client.archon.content_v1.deleteShareVersion(id, world, version),
	'Could not delete the version',
)
</script>

<template>
	<ShareHistory
		v-if="source"
		:versions="history.data.value ?? []"
		:loading="history.isPending.value"
		:editable="source.source === 'server'"
		:busy="restore.isPending.value || remove.isPending.value"
		subject="server"
		@restore="restore.mutate"
		@delete="remove.mutate"
	/>
</template>
