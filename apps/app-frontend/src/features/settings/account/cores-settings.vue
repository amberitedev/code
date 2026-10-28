<script setup lang="ts">
import type { Archon } from '@modrinth/api-client'
import { LinkIcon, UnlinkIcon, UpdatedIcon } from '@modrinth/assets'
import {
	Button,
	ConfirmModal,
	injectAuth,
	injectModrinthClient,
	injectNotificationManager,
	useRelativeTime,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { ref } from 'vue'

import AccountRequired from '@/features/cores/account-required.vue'
import { coresQueryKey, injectCoreActions, useCores } from '@/features/cores/use-cores'
import { coreOffline, serversQueryKey, useServers } from '@/features/servers/linked-servers'
import CoreStorage from '@/features/settings/account/core-storage.vue'

const client = injectModrinthClient()
const auth = injectAuth()
const queryClient = useQueryClient()
const { addNotification } = injectNotificationManager()
const formatRelativeTime = useRelativeTime()
// Both are kept fresh from the App root, so this page has its answer the moment it opens.
// Only the account's own Cores are managed here, not ones it was invited to.
const { ownCores: cores, data, isFetching, error, refetch } = useCores()
const { openAddCore } = injectCoreActions()
const { data: servers } = useServers()

/** Whether a Core answered the last server check. Undefined before the first check finishes. */
function isOnline(core: Archon.Cores.v1.Core) {
	if (coreOffline.value) return false
	return servers.value ? !servers.value.offline_cores?.includes(core.id) : undefined
}

const confirmRemove = ref<InstanceType<typeof ConfirmModal> | null>(null)
// The Core the confirmation is about, then the one being removed.
const selected = ref<Archon.Cores.v1.Core | null>(null)
const removingId = ref<string | null>(null)

function askRemove(core: Archon.Cores.v1.Core) {
	selected.value = core
	confirmRemove.value?.show()
}

async function remove() {
	const core = selected.value
	if (!core || removingId.value) return
	removingId.value = core.id
	try {
		await client.archon.cores_v1.revoke(core)
	} catch (cause) {
		console.error('Failed to remove Core', cause)
		addNotification({
			type: 'error',
			title: `Could not remove ${core.name}`,
			text: 'Check your connection, then try again.',
		})
	} finally {
		await Promise.all([
			queryClient.invalidateQueries({ queryKey: coresQueryKey }),
			queryClient.invalidateQueries({ queryKey: serversQueryKey }),
		])
		removingId.value = null
	}
}
</script>

<template>
	<AccountRequired
		v-if="!auth.user.value"
		description="Sign in to add a Core and host servers on a computer you control"
	/>
	<div v-else class="flex flex-col gap-4">
		<ConfirmModal
			ref="confirmRemove"
			:title="`Remove ${selected?.name ?? 'this Core'}?`"
			description="The Core is signed out of your account. Its servers and files stay on it, but you can't manage them from the App until you add it again. The Core will show a new code."
			proceed-label="Remove"
			:proceed-icon="UnlinkIcon"
			@proceed="remove"
		/>
		<div class="flex items-center justify-between gap-4">
			<div class="flex flex-col gap-1">
				<h2 class="m-0 text-lg font-semibold text-contrast">Cores</h2>
				<p class="m-0 text-secondary">A Core runs your servers on a computer you control.</p>
			</div>
			<Button type="colored" color="brand" @click="openAddCore()">
				<LinkIcon aria-hidden="true" /> Add Core
			</Button>
		</div>
		<div v-if="error && !data" class="flex items-center justify-between gap-4">
			<p class="m-0 text-red">Could not load your Cores. Check your connection, then try again.</p>
			<Button :disabled="isFetching" @click="refetch()">
				<UpdatedIcon aria-hidden="true" /> Try again
			</Button>
		</div>
		<p v-else-if="data && !cores.length" class="m-0 text-secondary">
			No Core yet. Start a Core, then enter the code it shows.
		</p>
		<div v-else class="flex flex-col">
			<div
				v-for="core in cores"
				:key="core.id"
				class="flex flex-col border-0 border-t border-solid border-surface-5 first:border-t-0"
			>
				<div class="flex items-center justify-between gap-4 py-3">
					<div class="flex min-w-0 flex-col gap-1">
						<span class="truncate font-semibold text-contrast">{{ core.name }}</span>
						<span class="flex min-w-0 items-center gap-1.5 text-sm text-secondary">
							<template v-if="isOnline(core) !== undefined">
								<span
									class="size-2 shrink-0 rounded-full"
									:class="isOnline(core) ? 'bg-green' : 'bg-red'"
									aria-hidden="true"
								/>
								{{ isOnline(core) ? 'Online' : 'Offline' }} ·
							</template>
							<span class="truncate">
								{{ core.address }} · Added {{ formatRelativeTime(core.created) }}
							</span>
						</span>
					</div>
					<Button :disabled="removingId !== null" @click="askRemove(core)">
						<UnlinkIcon aria-hidden="true" />
						{{ removingId === core.id ? 'Removing' : 'Remove' }}
					</Button>
				</div>
				<CoreStorage v-if="isOnline(core)" :core="core" />
			</div>
		</div>
	</div>
</template>
