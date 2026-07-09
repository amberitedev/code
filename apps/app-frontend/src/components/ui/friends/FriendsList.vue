<script setup lang="ts">
import type { AmberiteUser } from '@amberite/amberite-api'
import {
	CheckIcon,
	SendIcon,
	UserIcon,
	UserPlusIcon,
	UsersIcon,
	XIcon,
} from '@modrinth/assets'
import {
	Accordion,
	Avatar,
	ButtonStyled,
	GhostMedia,
	GhostText,
	injectNotificationManager,
	StyledInput,
	useRelativeTime,
} from '@modrinth/ui'
import dayjs from 'dayjs'
import { computed, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import FriendsSection from '@/components/ui/friends/FriendsSection.vue'
import ModalWrapper from '@/components/ui/modal/ModalWrapper.vue'
import { useSocial } from '@/composables/useSocial'
import type { FriendWithUserData } from '@/helpers/friends'

type PendingFriendWithRequest = FriendWithUserData & { requestId: string }

const { addNotification } = injectNotificationManager()
const formatRelativeTime = useRelativeTime()
const router = useRouter()
const {
	friends,
	currentUser,
	loading,
	error,
	group,
	members,
	invites,
	canManage,
	searchUsers,
	sendFriendRequest,
	respondFriendRequest,
	cancelFriendRequest,
	removeFriend,
	inviteToGroup,
	acceptInvite,
	declineInvite,
	blockUser,
	unblockUser,
} = useSocial()

const search = ref('')
const addFriendModal = ref()
const usernameQuery = ref('')
const userResults = ref<AmberiteUser[]>([])
const selectedUser = ref<AmberiteUser | null>(null)
const searchingUsers = ref(false)
const sending = ref(false)
const actingOnRequestId = ref<string | null>(null)
const actingOnGroupInviteId = ref<string | null>(null)
let searchToken = 0
let searchTimer: ReturnType<typeof setTimeout> | undefined

const friendList = computed(() => friends.value?.friends ?? [])
const incoming = computed(() => friends.value?.incoming ?? [])
const outgoing = computed(() => friends.value?.outgoing ?? [])
const groupMemberByUserId = computed(
	() => new Map(members.value.map((member) => [member.userId, member])),
)

const acceptedFriends = computed<FriendWithUserData[]>(() =>
	friendList.value.map((friend) => {
		const userId = friend.user?.userId ?? friend.friendshipId
		return {
			id: userId,
			friend_id: null,
			status: friend.presence?.status ?? null,
			last_updated: friend.presence?.lastSeenAt ? dayjs(friend.presence.lastSeenAt) : null,
			created: dayjs(friend.createdAt),
			username: friendLabel(friend),
			profileUsername: friend.user?.username ?? friend.user?.userId ?? null,
			accepted: true,
			online: !!friend.presence?.online,
			avatar: friend.user?.image ?? '',
			friendGroupRole: groupMemberByUserId.value.get(userId)?.role ?? null,
		}
	}),
)

const pendingFriends = computed<PendingFriendWithRequest[]>(() =>
	outgoing.value.map((request) => ({
		id: request.user?.userId ?? request.request.toUserId,
		friend_id: null,
		status: null,
		last_updated: null,
		created: dayjs(request.request.createdAt),
		username: requestLabel(request),
		profileUsername: request.user?.username ?? request.request.toUserId,
		accepted: false,
		online: false,
		avatar: request.user?.image ?? '',
		requestId: request.request._id,
	})),
)
const incomingRequests = computed<PendingFriendWithRequest[]>(() =>
	incoming.value.map((request) => ({
		id: request.user?.userId ?? request.request.fromUserId,
		friend_id: null,
		status: null,
		last_updated: null,
		created: dayjs(request.request.createdAt),
		username: requestLabel(request),
		profileUsername: request.user?.username ?? request.request.fromUserId,
		accepted: false,
		online: false,
		avatar: request.user?.image ?? '',
		requestId: request.request._id,
	})),
)
const blockedFriends = computed<FriendWithUserData[]>(() =>
	(friends.value?.blocks ?? []).map((block) => ({
		id: block.user?.userId ?? block.blockId,
		friend_id: null,
		status: null,
		last_updated: null,
		created: dayjs(block.createdAt),
		username: block.user?.displayName ?? block.user?.username ?? 'User',
		profileUsername: block.user?.username ?? block.user?.userId ?? null,
		accepted: true,
		online: false,
		avatar: block.user?.image ?? '',
	})),
)

const allFriends = computed(() => [
	...acceptedFriends.value,
	...pendingFriends.value,
	...incomingRequests.value,
])

const filteredFriends = computed<FriendWithUserData[]>(() => {
	const q = search.value.trim().toLowerCase()
	if (!q) return allFriends.value
	return allFriends.value.filter((f) => f.username.toLowerCase().includes(q))
})

const activeFriends = computed(() =>
	filteredFriends.value.filter((f) => !!f.status && f.online && f.accepted),
)
const onlineFriends = computed(() =>
	filteredFriends.value.filter((f) => f.online && !f.status && f.accepted),
)
const offlineFriends = computed(() => filteredFriends.value.filter((f) => !f.online && f.accepted))
const filteredPendingFriends = computed(() =>
	filteredFriends.value.filter(
		(friend) =>
			!friend.accepted &&
			pendingFriends.value.some((pending) => pending.requestId === (friend as PendingFriendWithRequest).requestId),
	),
)
const filteredIncomingRequests = computed(() => {
	const q = search.value.trim().toLowerCase()
	if (!q) return incomingRequests.value
	return incomingRequests.value.filter((friend) => friend.username.toLowerCase().includes(q))
})
const filteredGroupInvites = computed(() => {
	const q = search.value.trim().toLowerCase()
	if (!q) return invites.value
	return invites.value.filter((invite) => groupInviteName(invite).toLowerCase().includes(q))
})
const inviteCount = computed(() => filteredIncomingRequests.value.length + filteredGroupInvites.value.length)
const unavailableUserIds = computed(
	() =>
		new Set([
			...friendList.value.map((friend) => friend.user?.userId).filter(Boolean),
			...incoming.value.map((request) => request.request.fromUserId),
			...outgoing.value.map((request) => request.request.toUserId),
		]),
)

const canSendFriendRequest = computed(() => !!selectedUser.value && !sending.value)

function showFriendsServiceError(title = 'Friends unavailable') {
	addNotification({
		title,
		text: 'Amberite could not reach the account service. Check your connection and try again.',
		type: 'error',
	})
}

async function searchFriendUsers() {
	if (searchTimer) clearTimeout(searchTimer)
	const query = usernameQuery.value.trim()
	selectedUser.value = null
	if (query.length < 2) {
		userResults.value = []
		searchingUsers.value = false
		return
	}
	const token = ++searchToken
	searchingUsers.value = true
	searchTimer = setTimeout(async () => {
		try {
			const results = await searchUsers(query)
			if (token === searchToken) {
				userResults.value = results.filter((user) => !unavailableUserIds.value.has(user.userId))
			}
		} catch (e) {
			console.warn('[amberite] friend search failed', e)
			if (token === searchToken) userResults.value = []
			showFriendsServiceError('Search unavailable')
		} finally {
			if (token === searchToken) searchingUsers.value = false
		}
	}, 250)
}

onUnmounted(() => {
	if (searchTimer) clearTimeout(searchTimer)
})

function selectUser(user: AmberiteUser) {
	selectedUser.value = user
	usernameQuery.value = user.username ?? user.displayName ?? ''
	userResults.value = []
}

function openProfile(friend: FriendWithUserData) {
	const user = friend.profileUsername || friend.id || friend.username
	if (!user) return
	void router.push(`/user/${encodeURIComponent(user)}`)
}

async function add() {
	const user = selectedUser.value
	if (!user?.userId || !canSendFriendRequest.value) return
	sending.value = true
	try {
		await sendFriendRequest({ targetUserId: user.userId })
		if (error.value) {
			showFriendsServiceError('Friend request failed')
			return
		}
		usernameQuery.value = ''
		selectedUser.value = null
		userResults.value = []
		addFriendModal.value?.hide()
		addNotification({
			title: 'Friend request sent',
			text: `Sent a friend request to ${user.username ?? user.displayName ?? 'that user'}.`,
			type: 'success',
		})
	} catch (e) {
		console.warn('[amberite] friend request failed', e)
		showFriendsServiceError('Friend request failed')
	} finally {
		sending.value = false
	}
}

async function unfriend(userId: string) {
	await removeFriend(userId)
	if (error.value) {
		showFriendsServiceError('Friend update failed')
	}
}

async function inviteFriendToGroup(friend: FriendWithUserData) {
	await inviteToGroup({ inviteeUserId: friend.id })
	if (error.value) {
		showFriendsServiceError('Friend group invite failed')
		return
	}
	addNotification({
		title: 'Friend group invitation sent',
		text: `Invited ${friend.username} to your friend group.`,
		type: 'success',
	})
}

async function blockFriend(friend: FriendWithUserData) {
	await blockUser(friend.id)
	if (error.value) {
		showFriendsServiceError('Block failed')
		return
	}
	addNotification({
		title: 'Friend blocked',
		text: `${friend.username} was removed from your friends and blocked.`,
		type: 'success',
	})
}

async function unblockFriend(friend: FriendWithUserData) {
	await unblockUser(friend.id)
	if (error.value) {
		showFriendsServiceError('Unblock failed')
		return
	}
	addNotification({
		title: 'Friend unblocked',
		text: `${friend.username} can send you friend requests again.`,
		type: 'success',
	})
}

async function inviteFriendToPlay(friend: FriendWithUserData) {
	const connectionUrl = group.value?.core?.connectionUrl
	if (!connectionUrl) {
		addNotification({
			title: 'Play invitation unavailable',
			text: 'This Core does not have a connection link to share yet.',
			type: 'error',
		})
		return
	}
	await navigator.clipboard.writeText(connectionUrl)
	addNotification({
		title: 'Play invitation copied',
		text: `Share your Core connection link with ${friend.username} to invite them to play.`,
		type: 'success',
	})
}

async function removeMappedFriend(friend: FriendWithUserData) {
	if (!friend.accepted) {
		const requestId = (friend as Partial<PendingFriendWithRequest>).requestId
		if (requestId) {
			await cancelFriendRequest(requestId)
			if (error.value) {
				showFriendsServiceError('Friend request update failed')
			}
		}
		return
	}
	await unfriend(friend.id)
}

async function respondToRequest(friend: PendingFriendWithRequest, accept: boolean) {
	if (actingOnRequestId.value) return
	actingOnRequestId.value = friend.requestId
	try {
		await respondFriendRequest(friend.requestId, accept)
		if (error.value) {
			showFriendsServiceError('Friend request update failed')
			return
		}
		addNotification({
			title: accept ? 'Friend request accepted' : 'Friend request ignored',
			text: accept
				? `${friend.username} is now your friend.`
				: `Ignored the friend request from ${friend.username}.`,
			type: 'success',
		})
	} catch (e) {
		console.warn('[amberite] respond to friend request failed', e)
		showFriendsServiceError('Friend request update failed')
	} finally {
		actingOnRequestId.value = null
	}
}

async function respondToGroupInvite(invite: (typeof invites.value)[number], accept: boolean) {
	const inviteId = invite.invite._id
	if (!inviteId || actingOnGroupInviteId.value) return
	actingOnGroupInviteId.value = inviteId
	const groupName = groupInviteName(invite)
	try {
		if (accept) {
			await acceptInvite({ inviteId })
		} else {
			await declineInvite(inviteId)
		}
		if (error.value) {
			showFriendsServiceError('Friend group invite failed')
			return
		}
		addNotification({
			title: accept ? 'Friend group invite accepted' : 'Friend group invite ignored',
			text: accept ? `Joined ${groupName}.` : `Ignored the invite to ${groupName}.`,
			type: 'success',
		})
	} catch (e) {
		console.warn('[amberite] respond to friend group invite failed', e)
		showFriendsServiceError('Friend group invite failed')
	} finally {
		actingOnGroupInviteId.value = null
	}
}

function openGroupInvite(invite: (typeof invites.value)[number]) {
	const groupId = invite.group?.subdomain || invite.group?.id || invite.group?._id
	if (!groupId) return
	void router.push(`/group/${encodeURIComponent(groupId)}`)
}

function groupInviteName(invite: (typeof invites.value)[number]) {
	return invite.group?.name || invite.group?.subdomain || invite.group?.id || 'Friend group'
}

function friendLabel(f: (typeof friendList.value)[number]) {
	return f.user?.displayName ?? f.user?.username ?? 'User'
}

function requestLabel(req: (typeof outgoing.value)[number]) {
	return req.user?.displayName ?? req.user?.username ?? 'User'
}
</script>

<template>
	<ModalWrapper ref="addFriendModal" header="Adding a friend">
		<div class="min-w-[30rem]">
			<h2 class="m-0 text-base font-medium text-primary">
				What's your friend's Amberite username?
			</h2>
			<p class="m-0 mt-1 text-sm text-secondary leading-tight">
				Search for an existing Amberite user, then select them from the list.
			</p>
			<div class="relative mt-4">
				<StyledInput
					v-model="usernameQuery"
					:icon="UserIcon"
					type="text"
					placeholder="Search username..."
					wrapper-class="w-full"
					@update:model-value="searchFriendUsers"
					@keyup.enter="userResults.length === 1 ? selectUser(userResults[0]) : undefined"
				/>
				<div
					v-if="usernameQuery.trim().length >= 1 && !selectedUser"
					class="mt-2 max-h-56 w-full overflow-y-auto rounded-xl border border-solid border-surface-5 bg-bg-raised p-1"
				>
					<div v-if="usernameQuery.trim().length < 2" class="px-3 py-2 text-sm text-secondary">
						Type one more character to search.
					</div>
					<div v-else-if="searchingUsers" class="px-3 py-2 text-sm text-secondary">Searching...</div>
					<button
						v-for="user in usernameQuery.trim().length >= 2 ? userResults : []"
						:key="user.userId"
						class="grid w-full cursor-pointer grid-cols-[auto_1fr] items-center gap-2 rounded-lg border-0 bg-transparent px-2 py-2 text-left hover:bg-button-bg"
						@click="selectUser(user)"
					>
						<Avatar :src="user.image ?? ''" size="28px" circle />
						<span class="truncate text-sm text-primary">
							{{ user.displayName ?? user.username }}
						</span>
					</button>
					<div
						v-if="usernameQuery.trim().length >= 2 && !searchingUsers && userResults.length === 0"
						class="px-3 py-2 text-sm text-secondary"
					>
						No matching users.
					</div>
				</div>
			</div>
			<div class="flex justify-end mt-4">
				<ButtonStyled color="brand">
					<button :disabled="!canSendFriendRequest" @click="add">
						<SendIcon />
						{{ sending ? 'Sending...' : 'Send friend request' }}
					</button>
				</ButtonStyled>
			</div>
		</div>
	</ModalWrapper>
	<div class="flex flex-col h-full">
		<template v-if="currentUser">
			<div class="flex gap-1 items-center mb-3 -ml-1">
				<template v-if="allFriends.length > 0">
					<ButtonStyled circular type="transparent">
						<button
							v-tooltip="'Add a friend'"
							aria-label="Add a friend"
							@click="addFriendModal.show"
						>
							<UserPlusIcon />
						</button>
					</ButtonStyled>
					<StyledInput
						v-model="search"
						type="text"
						placeholder="Search friends..."
						clearable
						variant="outlined"
						wrapper-class="flex-1"
						@keyup.esc="search = ''"
					/>
				</template>
				<h3 v-else class="w-full text-base text-primary font-medium m-0">Friends</h3>
			</div>

			<div class="flex flex-col gap-3">
				<h3 v-if="loading" class="text-base text-primary font-medium m-0">Friends</h3>
			</div>

			<div v-if="loading" class="space-y-2">
				<div v-for="n in 4" :key="n" class="flex gap-2 items-center">
					<GhostMedia kind="circle" class="!w-9 shrink-0" />
					<div class="flex flex-col w-full gap-1">
						<GhostText kind="body" width="50%" />
						<GhostText kind="metadata" width="75%" />
					</div>
				</div>
			</div>

			<div v-else class="flex-1 overflow-y-auto space-y-3">
				<Accordion
					v-if="inviteCount > 0"
					:force-open="!!search"
					open-by-default
					:button-class="
						'flex w-full items-center bg-transparent border-0 p-0' +
						(search
							? ''
							: ' cursor-pointer hover:brightness-[--hover-brightness] active:scale-[0.98] transition-all')
					"
				>
					<template #title>
						<h3 class="text-base text-primary font-medium m-0">
							Invites - {{ inviteCount }}
						</h3>
					</template>
					<template #default>
						<div class="pt-3 flex flex-col gap-3">
							<div v-if="filteredGroupInvites.length > 0" class="flex flex-col gap-1">
								<h4
									v-if="filteredIncomingRequests.length > 0"
									class="m-0 px-2 text-xs font-semibold text-secondary"
								>
									Group invites
								</h4>
								<div
									v-for="invite in filteredGroupInvites"
									:key="invite.invite._id"
									class="group grid items-center grid-cols-[minmax(0,1fr)_auto] gap-2 hover:bg-button-bg transition-colors rounded-full mr-1"
								>
									<button
										type="button"
										class="grid min-w-0 cursor-pointer grid-cols-[auto_minmax(0,1fr)] items-center gap-2 rounded-full border-0 bg-transparent p-0 text-left focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand"
										@click="openGroupInvite(invite)"
									>
										<span
											class="flex h-12 w-12 shrink-0 items-center justify-center rounded-full bg-surface-3 text-secondary"
										>
											<UsersIcon class="size-5" aria-hidden="true" />
										</span>
										<div class="flex min-w-0 flex-col">
											<span class="m-0 truncate text-sm text-contrast">
												{{ groupInviteName(invite) }}
											</span>
											<span class="m-0 truncate text-xs text-secondary">
												Invited you to a Core -
												{{ formatRelativeTime(new Date(invite.invite.createdAt).toISOString()) }}
											</span>
										</div>
									</button>
									<div class="flex gap-1">
										<ButtonStyled circular type="transparent">
											<button
												v-tooltip="'Accept group invite'"
												:disabled="actingOnGroupInviteId !== null"
												aria-label="Accept group invite"
												@click="respondToGroupInvite(invite, true)"
											>
												<CheckIcon />
											</button>
										</ButtonStyled>
										<ButtonStyled circular type="transparent">
											<button
												v-tooltip="'Ignore group invite'"
												:disabled="actingOnGroupInviteId !== null"
												aria-label="Ignore group invite"
												@click="respondToGroupInvite(invite, false)"
											>
												<XIcon />
											</button>
										</ButtonStyled>
									</div>
								</div>
							</div>
							<div v-if="filteredIncomingRequests.length > 0" class="flex flex-col gap-1">
								<h4
									v-if="filteredGroupInvites.length > 0"
									class="m-0 px-2 text-xs font-semibold text-secondary"
								>
									Friend requests
								</h4>
								<div
									v-for="friend in filteredIncomingRequests"
									:key="friend.requestId"
									class="group grid items-center grid-cols-[minmax(0,1fr)_auto] gap-2 hover:bg-button-bg transition-colors rounded-full mr-1"
								>
									<button
										type="button"
										class="grid min-w-0 cursor-pointer grid-cols-[auto_minmax(0,1fr)] items-center gap-2 rounded-full border-0 bg-transparent p-0 text-left focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand"
										@click="openProfile(friend)"
									>
										<Avatar :src="friend.avatar" class="w-12 h-12 rounded-full" size="32px" circle />
										<div class="flex min-w-0 flex-col">
											<span class="m-0 truncate text-sm text-contrast">{{ friend.username }}</span>
											<span class="m-0 truncate text-xs text-secondary">
												Sent you a request - {{ formatRelativeTime(friend.created.toISOString()) }}
											</span>
										</div>
									</button>
									<div class="flex gap-1">
										<ButtonStyled circular type="transparent">
											<button
												v-tooltip="'Accept friend request'"
												:disabled="actingOnRequestId !== null"
												aria-label="Accept friend request"
												@click="respondToRequest(friend, true)"
											>
												<CheckIcon />
											</button>
										</ButtonStyled>
										<ButtonStyled circular type="transparent">
											<button
												v-tooltip="'Ignore friend request'"
												:disabled="actingOnRequestId !== null"
												aria-label="Ignore friend request"
												@click="respondToRequest(friend, false)"
											>
												<XIcon />
											</button>
										</ButtonStyled>
									</div>
								</div>
							</div>
						</div>
					</template>
				</Accordion>
				<div v-if="allFriends.length === 0 && !search" class="text-sm text-secondary">
					<button
						class="font-semibold text-brand cursor-pointer border-0 bg-transparent p-0"
						@click="addFriendModal.show"
					>
						Add friends
					</button>
					to see what they're playing!
				</div>
				<FriendsSection
					v-if="activeFriends.length > 0"
					:is-searching="!!search"
					open-by-default
					:friends="activeFriends"
					heading="Active"
					:remove-friend="removeMappedFriend"
					:open-profile="openProfile"
					:invite-to-group="group && canManage ? inviteFriendToGroup : undefined"
					:block-friend="blockFriend"
					:invite-to-play="group?.core ? inviteFriendToPlay : undefined"
				/>
				<FriendsSection
					v-if="onlineFriends.length > 0"
					:is-searching="!!search"
					open-by-default
					:friends="onlineFriends"
					heading="Online"
					:remove-friend="removeMappedFriend"
					:open-profile="openProfile"
					:invite-to-group="group && canManage ? inviteFriendToGroup : undefined"
					:block-friend="blockFriend"
					:invite-to-play="group?.core ? inviteFriendToPlay : undefined"
				/>
				<FriendsSection
					v-if="offlineFriends.length > 0"
					:is-searching="!!search"
					:open-by-default="activeFriends.length + onlineFriends.length < 3"
					:friends="offlineFriends"
					heading="Offline"
					:remove-friend="removeMappedFriend"
					:open-profile="openProfile"
					:invite-to-group="group && canManage ? inviteFriendToGroup : undefined"
					:block-friend="blockFriend"
					:invite-to-play="group?.core ? inviteFriendToPlay : undefined"
				/>
				<FriendsSection
					v-if="filteredPendingFriends.length > 0"
					:is-searching="!!search"
					:friends="filteredPendingFriends"
					heading="Pending"
					:remove-friend="removeMappedFriend"
					:open-profile="openProfile"
					:invite-to-group="group && canManage ? inviteFriendToGroup : undefined"
					:block-friend="blockFriend"
					:invite-to-play="group?.core ? inviteFriendToPlay : undefined"
				/>
				<FriendsSection
					v-if="blockedFriends.length > 0"
					:friends="blockedFriends"
					heading="Blocked"
					:remove-friend="unblockFriend"
					:open-profile="openProfile"
					:unblock-friend="unblockFriend"
				/>
				<p
					v-else-if="filteredFriends.length === 0 && inviteCount === 0 && search"
					class="text-sm text-secondary my-1 mx-4"
				>
					No friends matching '{{ search }}'
				</p>
			</div>
		</template>
	</div>
</template>
