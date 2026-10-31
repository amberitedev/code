<script setup lang="ts">
import { XIcon } from '@modrinth/assets'
import {
	CopyCode,
	defineMessages,
	IconButton,
	Table,
	type TableColumn,
	useFormatDateTime,
	useRelativeTime,
	useVIntl,
} from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref } from 'vue'

import { useSharedInstanceEvent } from '@/composables/use-shared-instance-event'
import { config } from '@/config'
import {
	get_shared_instance_invites,
	revoke_shared_instance_invite,
	type SharedInstanceInvite,
} from '@/helpers/instance'

import { injectInstancePage } from '../instance-context'
import { injectSharedInstance } from '../shared-instance-context'
import { injectSharedInstanceManagement } from './shared-instance-management-context'
import ConfirmRevokeSharedInstanceInviteModal from './confirm-revoke-shared-instance-invite-modal.vue'

// The owner's active invite links, under the members on the Share tab. Hidden while there are none.

const instancePage = injectInstancePage()
const sharedInstance = injectSharedInstance()
const management = injectSharedInstanceManagement()
const { formatMessage } = useVIntl()
const queryClient = useQueryClient()
const formatRelativeTime = useRelativeTime()
const formatDateTime = useFormatDateTime({ dateStyle: 'medium', timeStyle: 'short' })
const revokeInviteModal = ref<InstanceType<typeof ConfirmRevokeSharedInstanceInviteModal>>()
const instanceId = computed(() => instancePage.instance.value!.id)

const invitesQuery = useQuery({
	queryKey: computed(() => ['sharedInstanceInvites', instanceId.value] as const),
	queryFn: async ({ queryKey }) => {
		try {
			return await get_shared_instance_invites(queryKey[1])
		} catch (error) {
			void sharedInstance.reportActionError(error)
			throw error
		}
	},
	enabled: () => !instancePage.offline.value && !sharedInstance.shareActionsLocked.value,
	retry: false,
	staleTime: Infinity,
	refetchOnMount: 'always',
	refetchOnReconnect: false,
	refetchOnWindowFocus: false,
})
const invites = computed(() => invitesQuery.data.value ?? [])
// Someone joining through a link uses it up.
useSharedInstanceEvent(
	'shared_instance_users',
	() => instancePage.instance.value?.shared_instance?.id,
	() => void invitesQuery.refetch(),
)

const revokeMutation = useMutation({
	mutationFn: ({ instanceId, inviteId }: { instanceId: string; inviteId: string }) =>
		revoke_shared_instance_invite(instanceId, inviteId),
	onSuccess: (_data, { instanceId, inviteId }) => {
		queryClient.setQueryData<SharedInstanceInvite[]>(
			['sharedInstanceInvites', instanceId],
			(current = []) => current.filter((invite) => invite.id !== inviteId),
		)
		management.inviteRevoked(inviteId)
	},
	onError: (error) => void sharedInstance.reportActionError(error),
})

const columns = computed<TableColumn<'id' | 'uses' | 'expiration' | 'actions'>[]>(() => [
	{ key: 'id', label: formatMessage(messages.linkLabel), width: 'clamp(11rem, 34%, 19rem)' },
	{ key: 'uses', label: formatMessage(messages.usesLabel), width: 'clamp(7rem, 18%, 10rem)' },
	{
		key: 'expiration',
		label: formatMessage(messages.expiresLabel),
		width: 'clamp(9rem, 28%, 14rem)',
	},
	{ key: 'actions', label: '', align: 'right', width: '5.5rem' },
])

function revoke(inviteId: string) {
	if (!revokeMutation.isPending.value)
		revokeMutation.mutate({ instanceId: instanceId.value, inviteId })
}

const messages = defineMessages({
	title: { id: 'app.instance.share.invite-links.title', defaultMessage: 'Invite links' },
	linkLabel: { id: 'app.instance.share.invite-links.link', defaultMessage: 'Link' },
	usesLabel: { id: 'app.instance.share.invite-links.uses', defaultMessage: 'Uses' },
	expiresLabel: { id: 'app.instance.share.invite-links.expires', defaultMessage: 'Expires' },
	revoke: { id: 'app.instance.share.invite-links.revoke', defaultMessage: 'Revoke link' },
	revokeWithCode: {
		id: 'app.instance.share.invite-links.revoke-with-code',
		defaultMessage: 'Revoke link {code}',
	},
})
</script>

<template>
	<section v-if="invites.length" class="flex flex-col gap-3" aria-labelledby="invite-links-title">
		<ConfirmRevokeSharedInstanceInviteModal ref="revokeInviteModal" @revoke="revoke" />
		<h2 id="invite-links-title" class="m-0 text-lg font-semibold text-contrast">
			{{ formatMessage(messages.title) }}
		</h2>
		<Table :columns="columns" :data="invites" row-key="id" table-min-width="36rem">
			<template #cell-id="{ row }">
				<CopyCode
					:text="`${config.accountWebUrl || config.accountApiUrl || config.siteUrl}/share/${encodeURIComponent(row.id)}`"
					:display-text="`/${row.id}`"
				/>
			</template>
			<template #cell-uses="{ row }">
				<span class="font-medium text-primary">{{ row.uses }}</span>
				<span> / {{ row.maxUses }}</span>
			</template>
			<template #cell-expiration="{ row }">
				<span v-tooltip="formatDateTime(row.expiration)" class="whitespace-nowrap">
					{{ formatRelativeTime(row.expiration) }}
				</span>
			</template>
			<template #cell-actions="{ row }">
				<div class="flex justify-end">
					<IconButton
						v-tooltip="formatMessage(messages.revoke)"
						type="quiet"
						:label="formatMessage(messages.revokeWithCode, { code: row.id })"
						:disabled="revokeMutation.isPending.value || instancePage.offline.value"
						class="text-secondary hover:!filter-none hover:text-red focus-visible:!filter-none"
						@click="revokeInviteModal?.show(row.id)"
					>
						<XIcon aria-hidden="true" />
					</IconButton>
				</div>
			</template>
		</Table>
	</section>
</template>
