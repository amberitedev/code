<script setup lang="ts">
import { SendIcon, UserIcon, UserPlusIcon } from '@modrinth/assets'
import {
	Avatar,
	ButtonStyled,
	injectNotificationManager,
	StyledInput,
} from '@modrinth/ui'
import dayjs from 'dayjs'
import { computed, ref, watch } from 'vue'

import FriendsSection from '@/components/ui/friends/FriendsSection.vue'
import { useSocial } from '@/composables/useSocial'
import ModalWrapper from '@/components/ui/modal/ModalWrapper.vue'
import type { FriendWithUserData } from '@/helpers/friends'
import type { AmberiteUser } from '@amberite/amberite-api'

type PendingFriendWithRequest = FriendWithUserData & { requestId: string }

const { addNotification, handleError } = injectNotificationManager()
const {
	friends,
	currentUser,
	loading,
	error,
	searchUsers,
	sendFriendRequest,
	cancelFriendRequest,
	removeFriend,
} =
	useSocial()

const search = ref('')
const addFriendModal = ref()
const usernameQuery = ref('')
const userResults = ref<AmberiteUser[]>([])
const selectedUser = ref<AmberiteUser | null>(null)
const searchingUsers = ref(false)
const sending = ref(false)
let searchToken = 0

const friendList = computed(() => friends.value?.friends ?? [])
const incoming = computed(() => friends.value?.incoming ?? [])
const outgoing = computed(() => friends.value?.outgoing ?? [])

const acceptedFriends = computed<FriendWithUserData[]>(() =>
	friendList.value.map((friend) => ({
		id: friend.user?.userId ?? friend.friendshipId,
		friend_id: null,
		status: friend.presence?.status ?? null,
		last_updated: friend.presence?.lastSeenAt ? dayjs(friend.presence.lastSeenAt) : null,
		created: dayjs(friend.createdAt),
		username: friendLabel(friend),
		accepted: true,
		online: !!friend.presence?.online,
		avatar: friend.user?.image ?? '',
	})),
)

const pendingFriends = computed<PendingFriendWithRequest[]>(() =>
	outgoing.value.map((request) => ({
		id: request.user?.userId ?? request.request.toUserId,
		friend_id: null,
		status: null,
		last_updated: null,
		created: dayjs(request.request.createdAt),
		username: requestLabel(request),
		accepted: false,
		online: false,
		avatar: request.user?.image ?? '',
		requestId: request.request._id,
	})),
)

const allFriends = computed(() => [...acceptedFriends.value, ...pendingFriends.value])

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
const filteredPendingFriends = computed(() => filteredFriends.value.filter((f) => !f.accepted))
const unavailableUserIds = computed(
	() =>
		new Set([
			...friendList.value.map((friend) => friend.user?.userId).filter(Boolean),
			...incoming.value.map((request) => request.request.fromUserId),
			...outgoing.value.map((request) => request.request.toUserId),
		]),
)

const canSendFriendRequest = computed(() => !!selectedUser.value && !sending.value)

watch(error, (value) => {
	if (!value) return
	addNotification({
		title: 'Friends error',
		text: value.message,
		type: 'error',
	})
})

async function searchFriendUsers() {
	const query = usernameQuery.value.trim()
	selectedUser.value = null
	if (query.length < 3) {
		userResults.value = []
		return
	}
	const token = ++searchToken
	searchingUsers.value = true
	try {
		const results = await searchUsers(query)
		if (token === searchToken) {
			userResults.value = results.filter((user) => !unavailableUserIds.value.has(user.userId))
		}
	} catch (e) {
		if (token === searchToken) userResults.value = []
		handleError(e)
	} finally {
		if (token === searchToken) searchingUsers.value = false
	}
}

function selectUser(user: AmberiteUser) {
	selectedUser.value = user
	usernameQuery.value = user.username ?? user.displayName ?? ''
	userResults.value = []
}

async function add() {
	const user = selectedUser.value
	if (!user?.userId || !canSendFriendRequest.value) return
	sending.value = true
	try {
		await sendFriendRequest({ targetUserId: user.userId })
		if (error.value) {
			addNotification({
				title: 'Friend request failed',
				text: error.value.message,
				type: 'error',
			})
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
		handleError(e)
	} finally {
		sending.value = false
	}
}

async function unfriend(userId: string) {
	await removeFriend(userId).catch(handleError)
}

async function removeMappedFriend(friend: FriendWithUserData) {
	if (!friend.accepted) {
		const requestId = (friend as Partial<PendingFriendWithRequest>).requestId
		if (requestId) await cancelFriendRequest(requestId).catch(handleError)
		return
	}
	await unfriend(friend.id)
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
					v-if="usernameQuery.trim().length >= 3 && !selectedUser"
					class="mt-2 max-h-56 w-full overflow-y-auto rounded-xl border border-solid border-surface-5 bg-bg-raised p-1"
				>
					<div v-if="searchingUsers" class="px-3 py-2 text-sm text-secondary">Searching...</div>
					<button
						v-for="user in userResults"
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
						v-if="!searchingUsers && userResults.length === 0"
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
		<div v-if="!currentUser" class="text-sm text-secondary">
			<p class="m-0">Sign in to Minecraft to create your Amberite account and use friends.</p>
		</div>
		<template v-else>
			<div class="flex gap-1 items-center mb-3 -ml-1">
				<template v-if="allFriends.length > 0">
					<ButtonStyled circular type="transparent">
						<button v-tooltip="'Add a friend'" aria-label="Add a friend" @click="addFriendModal.show">
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
				<div v-for="n in 4" :key="n" class="flex gap-2 items-center animate-pulse">
					<div class="min-w-9 min-h-9 bg-button-bg rounded-full"></div>
					<div class="flex flex-col w-full">
						<div class="h-3 bg-button-bg rounded-full w-1/2 mb-1"></div>
						<div class="h-2.5 bg-button-bg rounded-full w-3/4"></div>
					</div>
				</div>
			</div>

			<div v-else class="flex-1 overflow-y-auto space-y-3">
				<FriendsSection
					v-if="activeFriends.length > 0"
					:is-searching="!!search"
					open-by-default
					:friends="activeFriends"
					heading="Active"
					:remove-friend="removeMappedFriend"
				/>
				<FriendsSection
					v-if="onlineFriends.length > 0"
					:is-searching="!!search"
					open-by-default
					:friends="onlineFriends"
					heading="Online"
					:remove-friend="removeMappedFriend"
				/>
				<FriendsSection
					v-if="offlineFriends.length > 0"
					:is-searching="!!search"
					:open-by-default="activeFriends.length + onlineFriends.length < 3"
					:friends="offlineFriends"
					heading="Offline"
					:remove-friend="removeMappedFriend"
				/>
				<FriendsSection
					v-if="filteredPendingFriends.length > 0"
					:is-searching="!!search"
					:friends="filteredPendingFriends"
					heading="Pending"
					:remove-friend="removeMappedFriend"
				/>
				<div v-if="allFriends.length === 0 && !search" class="text-sm text-secondary">
					<button
						class="font-semibold text-brand cursor-pointer border-0 bg-transparent p-0"
						@click="addFriendModal.show"
					>
						Add friends
					</button>
					to see what they're playing!
				</div>
				<p v-else-if="filteredFriends.length === 0 && search" class="text-sm text-secondary my-1 mx-4">
					No friends matching '{{ search }}'
				</p>
			</div>
		</template>
	</div>
</template>
