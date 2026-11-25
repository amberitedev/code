<template>
	<div class="relative flex flex-col gap-6">
		<ServerPlayCard
			:address="serverAddress"
			:disabled="actionsLocked || (!canSetup && !sharedInstanceId)"
			:pending-action="exporting ? 'download' : pendingAction"
			@play="perform('play')"
			@download="exportModpack"
		/>
		<section class="flex flex-col gap-3">
			<h1 class="m-0 text-2xl font-semibold text-contrast">
				{{ formatMessage(messages.invitedPlayersTitle) }}
			</h1>
			<Admonition
				v-if="players.members.isError.value"
				type="critical"
				:header="formatMessage(messages.playersError)"
			>
				<Button @click="players.members.refetch()">{{ formatMessage(messages.retry) }}</Button>
			</Admonition>
			<InvitedPlayersTable
				:rows="players.rows.value"
				:loading="players.members.isLoading.value"
				:can-manage="canSetup"
				:disabled="players.membershipMutation.isPending.value || actionsLocked"
				:show-push-update="!!sharedInstanceId && !followsInstance"
				:push-update-disabled="actionsLocked"
				:push-update-pending="previewQuery.isFetching.value || pendingAction === 'push'"
				:invite-disabled="actionsLocked"
				:invite-pending="pendingAction === 'invite'"
				@push-update="showPreview()"
				@push-update-hover="shareReview.prefetchFiles()"
				@invite="perform('invite')"
				@remove="confirmRemove"
			/>
		</section>
		<span
			ref="pageBottom"
			class="pointer-events-none absolute bottom-0 h-px w-px"
			aria-hidden="true"
		/>
		<InvitePlayersModal
			ref="invitePlayersModal"
			v-bind="inviteProps"
			:header="inviteHeader"
			@invite="invitePlayer"
			@cancel="(user) => changeMember(user.id, true)"
		/>
		<ServerSharePushModal
			ref="diffModal"
			:server-id="serverId"
			:world-id="serverId"
			:diffs="previewQuery.data.value?.items ?? []"
			:changes-description="
				previewAction === 'play' ? formatMessage(messages.shareChangesPlayBody) : undefined
			"
			:confirm-label="previewAction === 'play' ? formatMessage(messages.pushAndPlay) : undefined"
			:restart-confirm-label="
				previewAction === 'play' ? formatMessage(messages.pushRestartAndPlay) : undefined
			"
			:requires-restart="previewQuery.data.value?.diff.requires_restart"
			:loading="previewQuery.isFetching.value"
			:error="previewQuery.isError.value"
			:disabled="
				!canSetup || actionsLocked || previewQuery.isError.value || !previewQuery.data.value
			"
			@confirm="(paths, restart, removed) => perform(previewAction, true, paths, restart, removed)"
			@cancel="previewOpen = false"
			@retry="previewQuery.refetch()"
		/>
		<ConfirmModal
			ref="removeModal"
			:title="formatMessage(messages.removePlayer)"
			:description="
				formatMessage(messages.removeDescription, { username: playerToRemove?.username ?? '' })
			"
			:proceed-label="formatMessage(messages.removePlayer)"
			@proceed="removePlayer"
		/>
	</div>
</template>

<script setup lang="ts">
import { core as coreRoutes } from '@modrinth/api-client'
import { useIntersectionObserver } from '@vueuse/core'
import { computed, nextTick, onScopeDispose, ref, watch } from 'vue'

import Admonition from '#ui/components/base/Admonition.vue'
import { Button } from '#ui/components/base/buttons'
import ConfirmModal from '#ui/components/modal/ConfirmModal.vue'
import ServerSharePushModal from '#ui/components/servers/sharing/ServerSharePushModal.vue'
import {
	type InvitedPlayerRow,
	InvitedPlayersTable,
	type InviteLinkSettings,
	type InvitePlayersContentProps,
	type InvitePlayersInvitePayload,
	InvitePlayersModal,
	type InvitePlayersUser,
} from '#ui/components/sharing'
import {
	injectServerInviteHandoff,
	type ServerInviteBinding,
} from '#ui/components/sharing/server-sharing-update/handoff'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { useServerPermissions } from '#ui/composables/server-permissions'
import { inviteLinkUrl } from '#ui/composables/server-sharing/invite-link'
import { useServerOwnShare } from '#ui/composables/server-sharing/own-share'
import { mrpackFilename } from '#ui/composables/server-sharing/shared-instance'
import {
	type ServerShareActionTarget,
	useServerShareReview,
} from '#ui/composables/server-sharing/use-server-share-review'
import { useEnsureServerRunning } from '#ui/composables/use-ensure-server-running'
import {
	injectAuth,
	injectModrinthServerContext,
	injectNotificationManager,
	type ServerPlayTarget,
} from '#ui/providers'
import { useServerDownload } from '#ui/composables/server-download'
import { injectPageContext } from '#ui/providers/page-context'
import { getHostingServerAddress } from '#ui/utils/servers'

import ServerPlayCard from './ServerPlayCard.vue'
import { useServerPlayers } from './use-server-players'

type Action = 'play' | 'invite' | 'push'
const props = defineProps<{
	onPlayServer: (target: ServerPlayTarget) => void | Promise<void>
	siteUrl: string
}>()
const { formatMessage } = useVIntl()
const pageContext = injectPageContext(null)
const pageBottom = ref<HTMLElement | null>(null)
const intercomHiddenRequestId = Symbol('server-play-bottom')
useIntersectionObserver(pageBottom, ([entry]) => {
	pageContext?.intercomBubble?.requestHidden?.(
		intercomHiddenRequestId,
		entry?.isIntersecting ?? false,
	)
})
onScopeDispose(() => {
	pageContext?.intercomBubble?.requestHidden?.(intercomHiddenRequestId, false)
})
const { handleError } = injectNotificationManager()
const auth = injectAuth()
const { serverId, server } = injectModrinthServerContext()
const { canSetup } = useServerPermissions()
const ensureServerRunning = useEnsureServerRunning()
// The server's own share, or the one of the instance it follows.
const sharedInstanceId = computed(() => server.value.shared_instance_id)
const { needsUpdate } = useServerOwnShare()
const followsInstance = computed(() => !!server.value?.linked_instance)
const players = useServerPlayers(serverId, sharedInstanceId, canSetup)
const invitePlayersModal = ref<InstanceType<typeof InvitePlayersModal>>()
const inviteHandoff = injectServerInviteHandoff(null)
let inviteHandoffStarted = false
const shareReview = useServerShareReview<Action>({
	execute: performAction,
	disabled: computed(() => players.linkMutation.isPending.value),
})
const {
	diffModal,
	previewOpen,
	previewQuery,
	actionMutation,
	pending: actionsLocked,
	runAction,
} = shareReview
const removeModal = ref<InstanceType<typeof ConfirmModal>>()
const playerToRemove = ref<InvitedPlayerRow>()
const previewAction = ref<'play' | 'push'>('push')
const inviteHeader = computed(() =>
	formatMessage(messages.inviteHeader, { name: server.value.name }),
)
const inviteProps = computed<InvitePlayersContentProps>(() => ({
	friends: players.candidates.value,
	searchUsers: players.search,
	link: players.link.value ? inviteLinkUrl(props.siteUrl, players.link.value.id) : undefined,
	linkExpiresAt: players.link.value?.expiration,
	linkMaxUses: players.link.value?.max_uses,
	linkMaxUsesLimit: players.remaining.value,
	updateInviteLink,
	canInvite:
		canSetup.value &&
		!actionsLocked.value &&
		!players.membershipMutation.isPending.value &&
		players.remaining.value > 0,
	inviteDisabledMessage: formatMessage(messages.invitesUnavailable),
}))
const inviteBinding = computed<ServerInviteBinding>(() => ({
	header: inviteHeader.value,
	props: inviteProps.value,
	onInvite: invitePlayer,
	onCancel: (user) => changeMember(user.id, true),
}))
const serverAddress = computed(() => getHostingServerAddress(server.value.net))
async function performAction(action: Action, target: ServerShareActionTarget) {
	const { worldId: targetWorldId, configPaths, isCurrent: sameContext } = target
	let id = server.value.shared_instance_id
	// A server that follows an instance reads that instance's share, which only the instance
	// owner's App pushes: here it only manages players and plays.
	const shouldShare =
		!followsInstance.value &&
		canSetup.value &&
		(!id ||
			action === 'push' ||
			(action === 'play' && (needsUpdate.value || configPaths.length > 0)))
	if (shouldShare) {
		const shared = await shareReview.publish(target)
		id = shared.shared_instance_id
	} else if (!canSetup.value && (action === 'invite' || action === 'push')) {
		throw new Error(formatMessage(messages.permission))
	}
	if (!id) throw new Error(formatMessage(messages.notShared))
	if (!sameContext()) return
	if (action === 'play') {
		await ensureServerRunning(sameContext)
		if (!sameContext()) return
		await props.onPlayServer({ serverId, worldId: targetWorldId })
	} else if (action === 'invite') {
		await nextTick()
		const members = await players.members.refetch({ throwOnError: true })
		if (!sameContext()) return
		await players.ensureLink(id, members.data?.remaining)
		if (!sameContext()) return
		if (inviteHandoff?.requested.value) inviteHandoff.provide(inviteBinding)
		else invitePlayersModal.value?.show()
	}
}
const serverDownload = useServerDownload()
const exporting = ref(false)
/** Saves the server's content as a `.mrpack` its Core exports, for players on other launchers. */
async function exportModpack() {
	if (exporting.value) return
	exporting.value = true
	try {
		await serverDownload.download(
			'server-mrpack',
			mrpackFilename(server.value.name),
			coreRoutes.ExportModpack,
			{ params: { id: serverId } },
		)
	} finally {
		exporting.value = false
	}
}
const pendingAction = computed(() =>
	actionMutation.isPending.value ? actionMutation.variables.value?.action : undefined,
)
async function perform(
	action: Action,
	reviewed = false,
	configPaths: string[] = [],
	restart?: boolean,
	removedConfigPaths: string[] = [],
) {
	if (actionsLocked.value) return
	if (action === 'play' && !reviewed && canSetup.value && needsUpdate.value) {
		void showPreview(true)
		return
	}
	await runAction(action, reviewed, configPaths, restart, removedConfigPaths)
}
async function showPreview(playAfter = false) {
	if (actionsLocked.value) return
	previewAction.value = playAfter ? 'play' : 'push'
	await shareReview.showPreview(previewAction.value)
}
function changeMember(userId: string, remove: boolean, user?: InvitePlayersUser) {
	if (!sharedInstanceId.value || players.membershipMutation.isPending.value || !canSetup.value)
		return
	players.membershipMutation.mutate(
		{ id: sharedInstanceId.value, userId, remove, user },
		{ onError: (error) => handleError(error) },
	)
}
function invitePlayer(payload: InvitePlayersInvitePayload) {
	changeMember(payload.user.id, false, payload.user)
}
function confirmRemove(player: InvitedPlayerRow) {
	playerToRemove.value = player
	removeModal.value?.show()
}
function removePlayer() {
	if (playerToRemove.value) changeMember(playerToRemove.value.id, true)
}
async function updateInviteLink(settings: InviteLinkSettings) {
	if (!sharedInstanceId.value || players.linkMutation.isPending.value) return
	await players.linkMutation.mutateAsync({
		id: sharedInstanceId.value,
		settings,
		replaceId: players.link.value?.id,
	})
}
watch(
	[canSetup, () => inviteHandoff?.requested.value, actionsLocked],
	async () => {
		if (!inviteHandoff?.requested.value) {
			inviteHandoffStarted = false
			return
		}
		if (inviteHandoffStarted || actionsLocked.value) return
		inviteHandoffStarted = true
		if (canSetup.value) await perform('invite')
		if (!inviteHandoff.binding.value && !inviteHandoff.migration.value) inviteHandoff.cancel()
	},
	{ immediate: true },
)
onScopeDispose(() => {
	if (inviteHandoff?.binding.value === inviteBinding) inviteHandoff.cancel()
})
watch([sharedInstanceId, () => auth.user.value?.id], () => {
	invitePlayersModal.value?.hide()
	removeModal.value?.hide()
	playerToRemove.value = undefined
})
const messages = defineMessages({
	invitedPlayersTitle: { id: 'servers.play.players.title', defaultMessage: 'Invited players' },
	pushAndPlay: { id: 'servers.play.push-and-play', defaultMessage: 'Push update and play' },
	pushRestartAndPlay: {
		id: 'servers.play.push-restart-and-play',
		defaultMessage: 'Push, restart and play',
	},
	shareChangesPlayBody: {
		id: 'servers.play.share-changes-play-body',
		defaultMessage:
			'Starting the server will push these changes to all invited players. They will be asked to update their instance the next time they play.',
	},
	inviteHeader: { id: 'servers.play.invite-header', defaultMessage: 'Invite players to {name}' },
	playersError: {
		id: 'servers.play.players-error',
		defaultMessage: 'Could not load invited players',
	},
	retry: { id: 'servers.play.retry', defaultMessage: 'Retry' },
	invitesUnavailable: {
		id: 'servers.play.invites-unavailable',
		defaultMessage:
			'Invitations are unavailable while an action is in progress or the player limit has been reached.',
	},
	removePlayer: { id: 'servers.play.remove-player', defaultMessage: 'Remove player' },
	removeDescription: {
		id: 'servers.play.remove-description',
		defaultMessage:
			'Remove {username} from this server? They stop getting its content updates. This does not ban them from the Minecraft server.',
	},
	permission: {
		id: 'servers.play.permission',
		defaultMessage: 'You do not have permission to share this world.',
	},
	notShared: {
		id: 'servers.play.not-shared',
		defaultMessage: 'An owner or editor needs to share this world first.',
	},
})
</script>
