<script setup lang="ts">
import {
	injectModrinthClient,
	injectNotificationManager,
	SegmentedBar,
	useFormatBytes,
} from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, watch } from 'vue'

import {
	countedShares,
	sharedHistorySettingsKey,
	sharedStorageKey,
	useDeleteVersion,
	useSharedStorage,
	versionHistoryKey,
} from '@/features/settings/sharing/use-shared-storage'
import ShareHistory from '@/features/share-history/share-history.vue'
import { usePlayInstanceServer } from '@/features/share-history/use-play-instance-server'
import {
	getErrorMessage,
	install_get_shared_instance_history,
	install_restore_shared_instance_version,
	wait_for_install_job,
} from '@/helpers/install'
import { injectAppEvents } from '@/providers/app-events'

import { injectInstancePage } from '../instance-context'

// The owner's instance: the versions they pushed from this computer. The owner also sees this
// share's part of the allowance and can restore a version into this instance, to push again, or
// delete one. Restoring reads only this computer.
// A Play instance: its server's history, read from the server's Core, read-only, for the server's
// owner. The server is where its content is managed.

const instancePage = injectInstancePage()
const appEvents = injectAppEvents()
const client = injectModrinthClient()
const { addNotification } = injectNotificationManager()
const formatBytes = useFormatBytes()

const instance = computed(() => instancePage.instance.value!)
const shareId = computed(() => instance.value.shared_instance?.id)
const owner = computed(() => instance.value.shared_instance?.role === 'owner')
const playServer = usePlayInstanceServer(instancePage.instance)

const history = useQuery({
	queryKey: computed(() =>
		owner.value
			? versionHistoryKey(instance.value.id)
			: (['server-share-history', playServer.value?.serverId] as const),
	),
	queryFn: () => {
		const server = playServer.value
		return owner.value || !server
			? install_get_shared_instance_history(instance.value.id)
			: client.archon.content_v1.getShareHistory(server.serverId, server.worldId)
	},
	enabled: () => owner.value || !!playServer.value,
})
const versions = computed(() => history.data.value ?? [])

// A push adds a version and changes the share's usage.
const queryClient = useQueryClient()
watch(
	() => instance.value.shared_instance?.latest_version,
	() => {
		queryClient.invalidateQueries({ queryKey: versionHistoryKey(instance.value.id) })
		queryClient.invalidateQueries({ queryKey: sharedHistorySettingsKey })
		queryClient.invalidateQueries({ queryKey: sharedStorageKey })
	},
)

const { data: storage } = useSharedStorage()
const share = computed(() => storage.value?.shares.find((each) => each.id === shareId.value))
const segments = computed(() => {
	if (!storage.value || !share.value) return []
	const color = countedShares(storage.value).find((each) => each.id === share.value!.id)?.color
	return [
		{ id: 'this', label: 'This instance', value: share.value.used, color: color ?? 'bg-brand' },
		{
			id: 'others',
			label: 'Other instances',
			value: storage.value.used - share.value.used,
			color: 'bg-gray',
		},
	].map((segment) => ({ ...segment, display: formatBytes(segment.value, 1) }))
})

const deleteVersion = useDeleteVersion()
const restore = useMutation({
	mutationFn: async (version: number) => {
		const job = await install_restore_shared_instance_version(instance.value.id, version)
		await wait_for_install_job(appEvents, job.job_id)
	},
	onSuccess: () => instancePage.refreshInstance(),
	onError: (cause) => {
		console.error('Failed to restore version', cause)
		addNotification({
			type: 'error',
			title: 'Could not restore the version',
			text: getErrorMessage(cause),
		})
	},
})
const busy = computed(
	() =>
		restore.isPending.value ||
		deleteVersion.isPending.value ||
		instance.value.install_stage !== 'installed',
)
</script>

<template>
	<ShareHistory
		v-if="shareId"
		:versions="versions"
		:loading="history.isPending.value"
		:editable="owner"
		:busy="busy"
		subject="instance"
		@restore="restore.mutate"
		@delete="(version) => deleteVersion.mutate({ instanceId: instance.id, version })"
	>
		<div v-if="owner && storage && share?.counted" class="flex items-center gap-4">
			<SegmentedBar class="flex-1" :segments="segments" :max="storage.allowance" />
			<span class="shrink-0 font-semibold tabular-nums text-contrast">
				{{ formatBytes(share.used, 1) }} / {{ formatBytes(storage.allowance, 0) }}
			</span>
		</div>
	</ShareHistory>
</template>
