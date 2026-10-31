<template>
	<div v-if="!instance.quarantined" class="flex flex-col gap-4">
		<ModrinthAccountRequiredModal ref="accountRequiredModal" :request-auth="requestAuth" />
		<InvitePlayersModal
			ref="invitePlayersModal"
			:header="formatMessage(messages.shareModalHeader, { name: instance.name })"
			:friends="inviteFriends"
			:search-users="searchInviteUsers"
			:link="inviteLink.link.value"
			:link-expires-at="inviteLink.details.value?.expiresAt"
			:link-max-uses="inviteLink.details.value?.maxUses"
			:link-max-uses-limit="remainingUserSlots"
			:update-invite-link="inviteLink.update"
			:user-profile-link="userProfileLink"
			:can-invite="
				hasRemainingUserSlots &&
				!members.exclusiveMutationPending.value &&
				!inviteLink.pending.value
			"
			:invite-disabled-message="
				hasRemainingUserSlots
					? undefined
					: formatMessage(messages.userLimitReached, { limit: SHARED_INSTANCE_USER_LIMIT })
			"
			@invite="invitePlayer"
			@cancel="cancelInvite"
		/>
		<ConfirmUnlinkModal
			ref="unlinkModal"
			:warning="{
				header: formatMessage(messages.unlinkForShareHeader),
				body: formatMessage(messages.unlinkForShareBody),
			}"
			:backup-tip="importedModpackBackupTip"
			@unlink="unlinkImportedModpack"
		/>
		<SharedInstanceRemoveMemberModal
			ref="removeMemberModal"
			:row="pendingRemovalRow"
			:member-count="members.rows.value.length"
			@confirm="removeMember"
			@clear="pendingRemovalRow = null"
		/>
		<SharedInstanceShareEmptyState
			v-if="unableToConnect"
			:heading="formatMessage(messages.unableToConnectHeading)"
			:description="formatMessage(messages.unableToConnectDescription)"
		/>

		<SharedInstanceShareEmptyState
			v-else-if="sharedInstanceMissing"
			:heading="formatMessage(sharedInstanceErrorMessages.missingTitle)"
			:description="formatMessage(sharedInstanceErrorMessages.missingText)"
		>
			<template #actions>
				<Button
					type="outlined"
					size="lg"
					class="!border"
					:disabled="recovering"
					@click="stopSharing"
				>
					<UnlinkIcon aria-hidden="true" />{{
						formatMessage(sharedInstanceErrorMessages.stopSharingButton)
					}}
				</Button>
				<Button
					type="colored"
					color="brand"
					size="lg"
					:disabled="recovering || inviteLink.pending.value"
					@click="shareAgain($event)"
				>
					<SpinnerIcon
						v-if="recovering || inviteLink.pending.value"
						class="animate-spin"
						aria-hidden="true"
					/><UserPlusIcon v-else aria-hidden="true" />{{
						formatMessage(sharedInstanceErrorMessages.shareAgainButton)
					}}
				</Button>
			</template>
		</SharedInstanceShareEmptyState>

		<div v-else-if="membersTableLoading" class="h-64" aria-hidden="true" />

		<template v-else-if="showMembersTable">
			<SharedInstanceMembersTable />
			<SharedInstanceInviteLinks
				v-if="instance.shared_instance?.role === 'owner' && !sharedInstanceActionsLocked"
				class="mt-2"
			/>
		</template>

		<SharedInstanceShareEmptyState
			v-else-if="sharedInstanceUnavailable"
			:heading="formatMessage(sharedInstanceErrorMessages.unavailableTitle)"
			:description="
				formatSharedInstanceUnavailable(
					sharedInstanceUnavailableReason,
					sharedInstanceUnavailableManager,
				)
			"
		/>

		<SharedInstanceShareEmptyState
			v-else-if="sharedInstanceActionsLocked"
			:heading="formatMessage(lockedEmptyHeading)"
		>
			<template #description>
				<span class="flex flex-wrap items-center justify-center gap-x-1.5 gap-y-1">
					<span>{{ formatMessage(messages.lockedEmptyDescriptionPrefix) }}</span>
					<span
						v-if="linkedAccount"
						class="inline-flex max-w-full min-w-0 items-center gap-1.5 align-middle font-semibold text-primary"
					>
						<Avatar
							:src="linkedAccount.avatarUrl"
							:alt="linkedAccount.username"
							:tint-by="linkedAccount.tintBy"
							size="20px"
							circle
							no-shadow
						/>
						<span class="min-w-0 truncate">{{ linkedAccount.username }}</span>
					</span>
					<span v-else class="font-semibold text-primary">{{
						formatMessage(messages.linkedAccountFallback)
					}}</span>
					<span>{{ formatMessage(messages.lockedEmptyDescriptionSuffix) }}</span>
				</span>
			</template>
			<template #actions>
				<Button type="colored" color="brand" size="lg" @click="signInToShare">
					<LogInIcon aria-hidden="true" />{{ formatMessage(lockedActionButton) }}
				</Button>
			</template>
		</SharedInstanceShareEmptyState>

		<SharedInstanceShareEmptyState
			v-else
			:heading="formatMessage(messages.noFriendsInvitedHeading)"
			:description="formatMessage(messages.noFriendsInvitedDescription)"
		>
			<template #actions>
				<!-- Not shared yet, but a linked server still takes pushes. -->
				<InstancePushButton :instance="instance" />
				<Button
					type="colored"
					color="brand"
					size="lg"
					:disabled="inviteLink.pending.value || !hasRemainingUserSlots"
					@click="showInvitePlayers($event)"
				>
					<SpinnerIcon
						v-if="inviteLink.pending.value"
						class="animate-spin"
						aria-hidden="true"
					/><UserPlusIcon v-else aria-hidden="true" />{{
						formatMessage(messages.inviteFriendsButton)
					}}
				</Button>
			</template>
		</SharedInstanceShareEmptyState>
	</div>
</template>

<script setup lang="ts">
import { LogInIcon, SpinnerIcon, UnlinkIcon, UserPlusIcon } from '@modrinth/assets'
import {
	Avatar,
	Button,
	ConfirmUnlinkModal,
	defineMessages,
	injectAuth,
	injectNotificationManager,
	type InvitePlayersInvitePayload,
	InvitePlayersModal,
	type InvitePlayersUser,
	useVIntl,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'

import ModrinthAccountRequiredModal from '@/components/ui/modal/ModrinthAccountRequiredModal.vue'
import InstancePushButton from '@/features/servers/instance-push-button.vue'
import { isSharedInstancesApiError } from '@/helpers/install'
import { edit } from '@/helpers/instance'
import type { ModrinthAuthFlow } from '@/helpers/mr_auth.ts'
import {
	sharedInstanceErrorMessages,
	useSharedInstanceErrors,
} from '@/helpers/shared-instance-errors'

import { injectInstancePage } from '../instance-context'
import { injectSharedInstance } from '../shared-instance-context'
import SharedInstanceInviteLinks from './shared-instance-invite-links.vue'
import { provideSharedInstanceManagement } from './shared-instance-management-context'
import SharedInstanceMembersTable from './shared-instance-members-table.vue'
import SharedInstanceRemoveMemberModal from './shared-instance-remove-member-modal.vue'
import SharedInstanceShareEmptyState from './shared-instance-share-empty-state.vue'
import { SHARED_INSTANCE_USER_LIMIT, type ShareRow } from './shared-instance-share-types'
import { useSharedInstanceInviteCandidates } from './use-shared-instance-invite-candidates'
import { useSharedInstanceInviteLink } from './use-shared-instance-invite-link'
import { useSharedInstanceMembers } from './use-shared-instance-members'

const instancePage = injectInstancePage()
const auth = injectAuth()
const queryClient = useQueryClient()
const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()
const { formatSharedInstanceUnavailable, notifySharedInstanceError } = useSharedInstanceErrors()
const sharedInstanceState = injectSharedInstance()
const instance = computed(() => instancePage.instance.value!)
const offline = instancePage.offline
const actionsLocked = sharedInstanceState.shareActionsLocked
const sharedInstanceActionsLocked = actionsLocked
const currentUserId = computed(() => auth.user.value?.id ?? null)
const isSignedIn = computed(() => !!auth.session_token.value)
const sharedInstancesApiUnavailable = ref(false)
const accountRequiredModal = ref<InstanceType<typeof ModrinthAccountRequiredModal>>()
const invitePlayersModal = ref<InstanceType<typeof InvitePlayersModal>>()
const unlinkModal = ref<InstanceType<typeof ConfirmUnlinkModal>>()
const removeMemberModal = ref<InstanceType<typeof SharedInstanceRemoveMemberModal>>()
const pendingRemovalRow = ref<ShareRow | null>(null)
const importedModpackUnlinked = ref(false)
const recovering = ref(false)

function notifyOtherError(error: unknown) {
	if (isSharedInstancesApiError(error)) sharedInstancesApiUnavailable.value = true
	notifySharedInstanceError(error)
}

/** Every failed sharing action on this page ends up here. */
function notifyOperationError(error: unknown) {
	void sharedInstanceState.reportActionError(error, notifyOtherError)
}

function notifyInviteError(error: unknown, user: InvitePlayersUser) {
	void sharedInstanceState.reportActionError(error, (other) => {
		if (isSharedInstancesApiError(other)) return notifyOtherError(other)
		console.error(`Failed to invite ${user.username}`, other)
		addNotification({
			type: 'error',
			title: formatMessage(sharedInstanceErrorMessages.inviteFailedTitle),
			text: formatMessage(sharedInstanceErrorMessages.inviteFailedText, {
				username: user.username,
			}),
		})
	})
}

const eligibilityQuery = sharedInstanceState.eligibilityQuery

const members = useSharedInstanceMembers({
	instance,
	currentUserId,
	isSignedIn,
	actionsLocked,
	onError: notifyOperationError,
	onInviteError: notifyInviteError,
})
const remainingUserSlots = computed(() =>
	Math.max(0, SHARED_INSTANCE_USER_LIMIT - members.rows.value.length),
)
const hasRemainingUserSlots = computed(() => remainingUserSlots.value > 0)
const {
	inviteFriends,
	search: searchInviteUsers,
	requestFriend,
} = useSharedInstanceInviteCandidates({
	rows: members.rows,
	currentUserId,
	isSignedIn,
	actionsLocked,
})
const inviteLink = useSharedInstanceInviteLink(
	computed(() => instance.value.id),
	remainingUserSlots,
	notifyOperationError,
)

const linkedAccount = computed(() => {
	const manager = sharedInstanceState.manager.value
	return manager?.type === 'user'
		? { username: manager.name, avatarUrl: manager.avatarUrl, tintBy: manager.tintBy }
		: null
})
const lockedEmptyHeading = computed(() =>
	isSignedIn.value ? messages.lockedWrongAccountHeading : messages.lockedSignedOutHeading,
)
const lockedActionButton = computed(() =>
	isSignedIn.value ? messages.switchAccountButton : messages.signInButton,
)
const sharedInstanceUnavailableReason = sharedInstanceState.unavailableReason
const sharedInstanceUnavailable = computed(() => !!sharedInstanceUnavailableReason.value)
const sharedInstanceMissing = sharedInstanceState.missing
const sharedInstanceUnavailableManager = sharedInstanceState.unavailableManager
const unableToConnect = computed(
	() =>
		sharedInstancesApiUnavailable.value ||
		isSharedInstancesApiError(eligibilityQuery.error.value) ||
		isSharedInstancesApiError(members.query.error.value),
)
const membersTableLoading = computed(
	() =>
		members.rows.value.length === 0 &&
		!!instance.value.shared_instance &&
		(members.query.data.value === undefined || members.query.isFetching.value) &&
		!sharedInstanceUnavailable.value &&
		!sharedInstanceActionsLocked.value,
)
const showMembersTable = computed(
	() =>
		members.rows.value.length > 0 ||
		(!!instance.value.shared_instance &&
			members.query.data.value !== undefined &&
			!members.query.isFetching.value &&
			!sharedInstanceUnavailable.value &&
			!sharedInstanceActionsLocked.value),
)
const requiresUnlink = computed(
	() =>
		instance.value.link?.type === 'imported_modpack' &&
		!instance.value.shared_instance &&
		!importedModpackUnlinked.value,
)
const importedModpackBackupTip = computed(() =>
	instance.value.link?.type === 'imported_modpack'
		? (instance.value.link.name ?? instance.value.link.filename ?? undefined)
		: undefined,
)

const messages = defineMessages({
	signInButton: { id: 'app.instance.share.sign-in.button', defaultMessage: 'Sign in' },
	unableToConnectHeading: {
		id: 'app.instance.share.unable-to-connect.heading',
		defaultMessage: 'Unable to connect',
	},
	unableToConnectDescription: {
		id: 'app.instance.share.unable-to-connect.description',
		defaultMessage:
			'The shared instances service is not accessible at the moment, please try again later',
	},
	noFriendsInvitedHeading: {
		id: 'app.instance.share.empty.heading',
		defaultMessage: 'No friends invited',
	},
	noFriendsInvitedDescription: {
		id: 'app.instance.share.empty.description',
		defaultMessage: 'You can share this instance with your friends!',
	},
	inviteFriendsButton: {
		id: 'app.instance.share.empty.invite-friends-button',
		defaultMessage: 'Invite friends',
	},
	userLimitReached: {
		id: 'app.instance.share.invite-modal.user-limit-reached',
		defaultMessage: 'This instance has reached the {limit}-user limit.',
	},
	shareModalHeader: {
		id: 'app.instance.share.invite-modal.heading',
		defaultMessage: 'Share {name}',
	},
	lockedWrongAccountHeading: {
		id: 'app.instance.share.locked.wrong-account-heading',
		defaultMessage: 'Wrong account',
	},
	lockedSignedOutHeading: {
		id: 'app.instance.share.locked.signed-out-heading',
		defaultMessage: 'Not signed in',
	},
	lockedEmptyDescriptionPrefix: {
		id: 'app.instance.share.locked.empty-description-prefix',
		defaultMessage: 'You need to sign in as',
	},
	lockedEmptyDescriptionSuffix: {
		id: 'app.instance.share.locked.empty-description-suffix',
		defaultMessage: 'to access this page.',
	},
	linkedAccountFallback: {
		id: 'app.instance.share.locked.linked-account-fallback',
		defaultMessage: 'the linked account',
	},
	switchAccountButton: {
		id: 'app.instance.share.locked.switch-account-button',
		defaultMessage: 'Switch account',
	},
	unlinkForShareHeader: {
		id: 'app.instance.share.unlink.header',
		defaultMessage: 'Sharing requires unlinking',
	},
	unlinkForShareBody: {
		id: 'app.instance.share.unlink.body',
		defaultMessage: 'You must unlink this modpack to share your instance',
	},
})

function invitePlayer(payload: InvitePlayersInvitePayload) {
	if (actionsLocked.value || !hasRemainingUserSlots.value) return
	if (payload.source === 'search') void requestFriend(payload.user)
	members.invite(payload.user)
}
function cancelInvite(user: InvitePlayersUser) {
	const row = members.find(user.id, user.username)
	if (row) members.remove(row.id)
}
async function showInvitePlayers(event?: MouseEvent) {
	if (actionsLocked.value) return
	if (!isSignedIn.value) return signInToShare(event)
	if (!hasRemainingUserSlots.value) return
	if (requiresUnlink.value) return unlinkModal.value?.show()
	if (await inviteLink.ensure()) invitePlayersModal.value?.show(event)
}
async function unlinkImportedModpack() {
	try {
		await edit(instance.value.id, { link: null as unknown as undefined })
		importedModpackUnlinked.value = true
		await queryClient.invalidateQueries({ queryKey: ['linkedModpackInfo', instance.value.id] })
		if (await inviteLink.ensure()) invitePlayersModal.value?.show()
	} catch (error) {
		notifyOperationError(error)
	}
}
async function stopSharing() {
	recovering.value = true
	try {
		await sharedInstanceState.stopSharing()
		inviteLink.reset()
		return true
	} catch (error) {
		notifyOperationError(error)
		return false
	} finally {
		recovering.value = false
	}
}
/** Detaching first makes the usual first share create a fresh shared instance and publish to it. */
async function shareAgain(event?: MouseEvent) {
	if (await stopSharing()) await showInvitePlayers(event)
}
function showRemoveMemberModal(row: ShareRow) {
	if (!actionsLocked.value) {
		pendingRemovalRow.value = row
		removeMemberModal.value?.show()
	}
}
function removeMember(row: ShareRow) {
	members.remove(row.id)
}
function userProfileLink(username: string) {
	return !username || username.includes('@') ? undefined : `/user/${encodeURIComponent(username)}`
}
async function requestAuth(flow: ModrinthAuthFlow) {
	await auth.requestSignIn(`/instance/${encodeURIComponent(instance.value.id)}/share`, flow, {
		showModal: false,
	})
	return !!auth.session_token.value
}
function signInToShare(event?: MouseEvent) {
	void accountRequiredModal.value?.show(event)
}

provideSharedInstanceManagement({
	rows: members.rows,
	actionsLocked: sharedInstanceActionsLocked,
	inviteDisabled: computed(() => !hasRemainingUserSlots.value),
	invitePending: inviteLink.pending,
	pushUpdateDisabled: computed(() => offline.value),
	invite: (event) => void showInvitePlayers(event),
	inviteRevoked: (inviteId) => {
		if (inviteLink.details.value?.inviteId === inviteId) inviteLink.reset()
	},
	remove: showRemoveMemberModal,
})

watch(
	[eligibilityQuery.error, members.query.error],
	(errors) => {
		for (const error of errors) {
			if (isSharedInstancesApiError(error)) notifyOperationError(error)
		}
		// The member list refusing to load usually means the shared instance is gone or not ours.
		const membersError = errors[1]
		if (membersError && !isSharedInstancesApiError(membersError)) {
			notifyOperationError(membersError)
		}
	},
	{ immediate: true },
)
// Opening the page checks that the backend still has this shared instance and that we own it.
watch([() => instance.value.id, currentUserId], () => void sharedInstanceState.verifyOwner(), {
	immediate: true,
})
watch([eligibilityQuery.data, members.query.data], ([eligibility, memberRows]) => {
	if (eligibility !== undefined && memberRows !== undefined) {
		sharedInstancesApiUnavailable.value = false
	}
})
watch(
	() => instance.value.id,
	() => {
		importedModpackUnlinked.value = false
	},
)
watch(
	[() => auth.isReady!.value, isSignedIn, actionsLocked],
	([ready, signedIn, locked]) => {
		if (ready && !signedIn && !locked) signInToShare()
	},
	{ immediate: true, flush: 'post' },
)
</script>
