<script setup lang="ts">
import { injectModrinthServerContext, injectNotificationManager, serverKeys } from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import ShareHistory from '@/features/share-history/share-history.vue'
import { useServerHistory } from '@/features/share-history/use-server-history'
import { useShareHistory } from '@/features/share-history/use-share-history'
import { getErrorMessage } from '@/helpers/install'
import { apiClient } from '@/platform/modrinth-client'

// A server's History tab. A server-first server's history is on its Core, which restores and
// deletes. A linked server shows its instance's history from this App, read-only; restoring and
// pushing happen on the instance.

const route = useRoute()
const queryClient = useQueryClient()
const { addNotification } = injectNotificationManager()
const { core, server } = injectModrinthServerContext()

const serverId = computed(() => String(route.params.id ?? ''))
const source = useServerHistory(serverId)
const { history, restore, remove } = useShareHistory(() => {
	const from = source.value
	if (!from) return null
	return from.source === 'server'
		? { client: core, instanceId: from.serverId }
		: { client: apiClient.local, instanceId: from.instanceId }
})

function reportError(title: string) {
	return (cause: unknown) => {
		console.error(title, cause)
		addNotification({ type: 'error', title, text: getErrorMessage(cause) })
	}
}

/** What a change of the server's history changes besides it. */
function reloadServer() {
	return Promise.all([
		queryClient.invalidateQueries({ queryKey: serverKeys.content(serverId.value) }),
		queryClient.invalidateQueries({ queryKey: serverKeys.detail(serverId.value) }),
		queryClient.invalidateQueries({ queryKey: serverKeys.shareDiff(serverId.value) }),
	])
}

function restoreVersion(version: number) {
	restore.mutate(version, {
		onError: reportError('Could not restore the version'),
		onSettled: reloadServer,
	})
}

function deleteVersion(version: number) {
	remove.mutate(version, {
		onError: reportError('Could not delete the version'),
		onSettled: reloadServer,
	})
}

// A restore onto another loader or pack goes on in the background, holding the server, which
// reports itself installing meanwhile.
const busy = computed(
	() => restore.isPending.value || remove.isPending.value || server.value?.status === 'installing',
)
</script>

<template>
	<ShareHistory
		v-if="source"
		:versions="history.data.value ?? []"
		:loading="history.isPending.value"
		:editable="source.source === 'server'"
		:busy="busy"
		subject="server"
		@restore="restoreVersion"
		@delete="deleteVersion"
	/>
</template>
