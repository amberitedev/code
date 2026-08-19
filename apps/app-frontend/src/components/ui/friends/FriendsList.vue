<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { MailIcon, SearchIcon, SendIcon, UserIcon, UserPlusIcon, XIcon } from '@modrinth/assets'
import {
	Avatar,
	Button,
	defineMessages,
	IconButton,
	injectNotificationManager,
	IntlFormatted,
	StyledInput,
	useRelativeTime,
	useVIntl,
} from '@modrinth/ui'
import { computed, onUnmounted, ref } from 'vue'

import FriendsSection from '@/components/ui/friends/FriendsSection.vue'
import ModalWrapper from '@/components/ui/modal/ModalWrapper.vue'
import { useFriends } from '@/composables/use-friends'
import type { FriendWithUserData } from '@/helpers/friends.ts'
import type { SocialCredentials } from '@/helpers/friends.ts'
import { get as getSettings, set as setSettings } from '@/helpers/settings.ts'
import { useTheming } from '@/store/state'
import { apiClient } from '@/services/api-client'

const { formatMessage } = useVIntl()

const { handleError } = injectNotificationManager()
const formatRelativeTime = useRelativeTime()
const themeStore = useTheming()

const props = defineProps<{
	credentials: SocialCredentials | null
	signIn: () => void
}>()

type FriendsSectionCollapsedFlag =
	| 'friends_active_collapsed'
	| 'friends_online_collapsed'
	| 'friends_offline_collapsed'
	| 'friends_pending_collapsed'

function isFriendsSectionCollapsed(flag: FriendsSectionCollapsedFlag) {
	return themeStore.getFeatureFlag(flag)
}

function setFriendsSectionCollapsed(flag: FriendsSectionCollapsedFlag, collapsed: boolean) {
	themeStore.featureFlags[flag] = collapsed
	getSettings()
		.then((settings) => {
			settings.feature_flags[flag] = collapsed
			return setSettings(settings)
		})
		.catch(handleError)
}

const userCredentials = computed(() => props.credentials)
const {
	friends: userFriends,
	query: friendsQuery,
	loading,
	requestFriend,
	acceptFriend,
	removeFriend: removeFriendRecord,
} = useFriends({
	currentUserId: () => userCredentials.value?.user_id,
	getCredentials: () => userCredentials.value,
	onError: handleError,
})

const search = ref('')
const friendInvitesModal = ref()
const username = ref('')
const friendCode = ref('')
const addingByCode = ref(false)
const addFriendModal = ref()
const userResults = ref<Labrinth.Users.v3.SearchUser[]>([])
const selectedUser = ref<Labrinth.Users.v3.SearchUser | null>(null)
const searchingUsers = ref(false)
let searchRequest = 0
let searchTimer: ReturnType<typeof setTimeout> | null = null

const sortedFriends = computed<FriendWithUserData[]>(() =>
	userFriends.value.slice().sort((a, b) => {
		if (a.last_updated === null && b.last_updated === null) {
			return 0 // Both are null, equal in sorting
		}
		if (a.last_updated === null) {
			return 1 // `a` is null, move it after `b`
		}
		if (b.last_updated === null) {
			return -1 // `b` is null, move it after `a`
		}
		// Both are non-null, sort by date
		return b.last_updated.diff(a.last_updated)
	}),
)
const filteredFriends = computed<FriendWithUserData[]>(() =>
	sortedFriends.value.filter((friend) =>
		[friend.displayName, friend.username].some((value) =>
			value.trim().toLowerCase().includes(search.value.trim().toLowerCase()),
		),
	),
)

const activeFriends = computed<FriendWithUserData[]>(() =>
	filteredFriends.value.filter((x) => !!x.status && x.online && x.accepted),
)
const onlineFriends = computed<FriendWithUserData[]>(() =>
	filteredFriends.value.filter((x) => x.online && !x.status && x.accepted),
)
const offlineFriends = computed<FriendWithUserData[]>(() =>
	filteredFriends.value.filter((x) => !x.online && x.accepted),
)
const pendingFriends = computed(() =>
	filteredFriends.value
		.filter((x) => !x.accepted && x.id !== userCredentials.value?.user_id)
		.slice()
		.sort((a, b) => b.created.diff(a.created)),
)
const incomingRequests = computed(() =>
	userFriends.value
		.filter((x) => !x.accepted && x.id === userCredentials.value?.user_id)
		.slice()
		.sort((a, b) => b.created.diff(a.created)),
)

function addFriendFromModal() {
	if (!selectedUser.value) return

	addFriendModal.value.hide()
	requestFriend({
		id: selectedUser.value.id,
		username: selectedUser.value.username,
		displayName: selectedUser.value.display_name,
		avatarUrl: selectedUser.value.avatar_url,
	})
	username.value = ''
	selectedUser.value = null
	userResults.value = []
}

async function addFriendByCode() {
	const code = friendCode.value.trim().toUpperCase()
	if (!/^AMB-[A-Z0-9]{8}$/.test(code)) return
	addingByCode.value = true
	try {
		await apiClient.amberite.friends_v1.addByCode(code)
		friendCode.value = ''
		addFriendModal.value.hide()
		await friendsQuery.refetch()
	} catch (error) {
		handleError(error instanceof Error ? error : new Error(String(error)))
	} finally {
		addingByCode.value = false
	}
}

function searchUsers() {
	if (searchTimer) clearTimeout(searchTimer)
	selectedUser.value = null
	const query = username.value.trim()
	if (query.length < 2) {
		userResults.value = []
		searchingUsers.value = false
		return
	}
	const request = ++searchRequest
	searchingUsers.value = true
	searchTimer = setTimeout(async () => {
		try {
			const results = await apiClient.labrinth.users_v3.search(query)
			if (request === searchRequest) userResults.value = results
		} finally {
			if (request === searchRequest) searchingUsers.value = false
		}
	}, 250)
}

function selectUser(user: Labrinth.Users.v3.SearchUser) {
	selectedUser.value = user
	username.value = `@${user.username}`
	userResults.value = []
}

onUnmounted(() => {
	if (searchTimer) clearTimeout(searchTimer)
})

async function addFriend(friend: FriendWithUserData) {
	acceptFriend(friend)
}

async function removeFriend(friend: FriendWithUserData) {
	removeFriendRecord(friend)
}

const messages = defineMessages({
	addFriend: {
		id: 'friends.action.add-friend',
		defaultMessage: 'Add a friend',
	},
	addingAFriend: {
		id: 'friends.add-friend.title',
		defaultMessage: 'Adding a friend',
	},
	usernameTitle: {
		id: 'friends.add-friend.username.title',
		defaultMessage: "What's your friend's Minecraft username?",
	},
	usernameDescription: {
		id: 'friends.add-friend.username.description',
		defaultMessage: 'Search by their unique @username, then choose the matching profile.',
	},
	usernamePlaceholder: {
		id: 'friends.add-friend.username.placeholder',
		defaultMessage: 'Search @username...',
	},
	friendCodeTitle: {
		id: 'friends.add-friend.code.title',
		defaultMessage: 'Or use a friend code',
	},
	friendCodeDescription: {
		id: 'friends.add-friend.code.description',
		defaultMessage: 'Friend codes work even when you do not know someone’s current username.',
	},
	friendCodePlaceholder: {
		id: 'friends.add-friend.code.placeholder',
		defaultMessage: 'AMB-XXXXXXXX',
	},
	sendFriendRequest: {
		id: 'friends.add-friend.submit',
		defaultMessage: 'Send friend request',
	},
	viewFriendRequests: {
		id: 'friends.action.view-friend-requests',
		defaultMessage: '{count} friend {count, plural, one {request} other {requests}}',
	},
	searchFriends: {
		id: 'friends.search-friends-placeholder',
		defaultMessage: 'Search friends...',
	},
	friends: {
		id: 'friends.heading',
		defaultMessage: 'Friends',
	},
	pending: {
		id: 'friends.heading.pending',
		defaultMessage: 'Pending',
	},
	active: {
		id: 'friends.heading.active',
		defaultMessage: 'Active',
	},
	online: {
		id: 'friends.heading.online',
		defaultMessage: 'Online',
	},
	offline: {
		id: 'friends.heading.offline',
		defaultMessage: 'Offline',
	},
	noFriendsMatch: {
		id: 'friends.no-friends-match',
		defaultMessage: `No friends matching ''{query}''`,
	},
	signInToAddFriends: {
		id: 'friends.sign-in-to-add-friends',
		defaultMessage: "<link>Sign in to Amberite</link> to add friends and see who's online!",
	},
	addFriendsToShare: {
		id: 'friends.add-friends-to-share',
		defaultMessage: "<link>Add friends</link> to see what they're playing!",
	},
})
</script>

<template>
	<ModalWrapper ref="friendInvitesModal" header="View friend requests">
		<p v-if="incomingRequests.length === 0">You have no pending friend requests :C</p>
		<div v-else class="flex flex-col gap-4 min-w-[40rem]">
			<div v-for="friend in incomingRequests" :key="friend.username" class="flex gap-2">
				<Avatar :src="friend.avatar" class="w-12 h-12 rounded-full" size="2.25rem" circle />
				<div class="grid grid-cols-[1fr_auto] w-full gap-4">
					<div>
						<p class="m-0">
							<template v-if="friend.id === userCredentials?.user_id">
								<span class="text-contrast">{{ friend.displayName }}</span> sent you a friend
								request
							</template>
							<template v-else>
								You sent <span class="font-bold">{{ friend.displayName }}</span> a friend request
							</template>
						</p>
						<p class="m-0 text-xs text-secondary">@{{ friend.username }}</p>
						<p class="m-0 text-sm text-secondary">
							{{ formatRelativeTime(friend.created.toISOString()) }}
						</p>
					</div>
					<div class="flex gap-2">
						<template v-if="friend.id === userCredentials?.user_id">
							<Button type="colored" color="brand" @click="addFriend(friend)">
								<UserPlusIcon />
								Accept
							</Button>
							<Button @click="removeFriend(friend)">
								<XIcon />
								Ignore
							</Button>
						</template>
						<template v-else>
							<Button @click="removeFriend(friend)">
								<XIcon />
								Cancel
							</Button>
						</template>
					</div>
				</div>
			</div>
		</div>
	</ModalWrapper>
	<ModalWrapper ref="addFriendModal" :header="formatMessage(messages.addingAFriend)">
		<div class="min-w-[30rem] flex flex-col gap-5">
			<div
				v-if="userCredentials?.user?.friendCode"
				class="rounded-xl bg-surface-2 px-4 py-3 text-sm text-secondary"
			>
				Your friend code:
				<span class="ml-1 font-mono font-semibold text-contrast">
					{{ userCredentials.user.friendCode }}
				</span>
			</div>
			<div>
				<h2 class="m-0 text-base font-medium text-primary">
					{{ formatMessage(messages.usernameTitle) }}
				</h2>
				<p class="m-0 mt-1 text-sm text-secondary leading-tight">
					{{ formatMessage(messages.usernameDescription) }}
				</p>
				<div class="flex items-center gap-2 mt-4">
					<StyledInput
						v-model="username"
						:icon="UserIcon"
						type="text"
						:placeholder="formatMessage(messages.usernamePlaceholder)"
						wrapper-class="flex-1"
						@update:model-value="searchUsers"
						@keyup.enter="
							userResults.length === 1 ? selectUser(userResults[0]) : addFriendFromModal()
						"
					/>
					<Button
						type="colored"
						color="brand"
						:disabled="!selectedUser"
						@click="addFriendFromModal"
					>
						<SendIcon />
						{{ formatMessage(messages.sendFriendRequest) }}
					</Button>
				</div>
				<div
					v-if="username.trim().length > 0 && !selectedUser"
					class="mt-2 max-h-56 overflow-y-auto rounded-xl border border-solid border-surface-5 bg-bg-raised p-1"
				>
					<div v-if="username.trim().length < 2" class="px-3 py-2 text-sm text-secondary">
						Type at least two characters.
					</div>
					<div v-else-if="searchingUsers" class="px-3 py-2 text-sm text-secondary">Searching…</div>
					<template v-else>
						<button
							v-for="user in userResults"
							:key="user.id"
							type="button"
							class="flex w-full items-center gap-3 rounded-lg border-0 bg-transparent px-3 py-2 text-left hover:bg-button-bg"
							@click="selectUser(user)"
						>
							<Avatar :src="user.avatar_url" size="32px" circle />
							<span class="min-w-0">
								<span class="block truncate font-semibold text-contrast">
									{{ user.display_name ?? user.username }}
								</span>
								<span class="block truncate text-xs text-secondary">@{{ user.username }}</span>
							</span>
						</button>
					</template>
					<div
						v-if="!searchingUsers && username.trim().length >= 2 && userResults.length === 0"
						class="px-3 py-2 text-sm text-secondary"
					>
						No matching users.
					</div>
				</div>
			</div>
			<div class="border-0 border-t border-solid border-surface-5 pt-5">
				<h2 class="m-0 text-base font-medium text-primary">
					{{ formatMessage(messages.friendCodeTitle) }}
				</h2>
				<p class="m-0 mt-1 text-sm text-secondary leading-tight">
					{{ formatMessage(messages.friendCodeDescription) }}
				</p>
				<div class="mt-4 flex items-center gap-2">
					<StyledInput
						v-model="friendCode"
						class="flex-1 font-mono uppercase"
						:maxlength="12"
						:placeholder="formatMessage(messages.friendCodePlaceholder)"
						@keydown.enter="addFriendByCode"
					/>
					<Button
						type="colored"
						color="brand"
						:disabled="addingByCode || !/^AMB-[A-Z0-9]{8}$/i.test(friendCode.trim())"
						@click="addFriendByCode"
					>
						<UserPlusIcon /> {{ addingByCode ? 'Adding…' : 'Add' }}
					</Button>
				</div>
			</div>
		</div>
	</ModalWrapper>
	<div v-if="userCredentials && !loading" class="flex gap-1 items-center mb-3 -ml-1">
		<template v-if="sortedFriends.length > 0">
			<IconButton
				v-tooltip="formatMessage(messages.addFriend)"
				type="quiet"
				:label="formatMessage(messages.addFriend)"
				@click="addFriendModal.show"
			>
				<UserPlusIcon />
			</IconButton>
			<StyledInput
				v-model="search"
				:icon="SearchIcon"
				type="text"
				:placeholder="formatMessage(messages.searchFriends)"
				clearable
				input-class="!bg-transparent !border !border-solid !border-button-bg !text-primary !placeholder:text-primary"
				wrapper-class="flex-1 [&>svg]:!text-primary [&>svg]:!opacity-100"
				@keyup.esc="search = ''"
			/>
		</template>
		<h3 v-else class="w-full text-base text-primary font-medium m-0">
			{{ formatMessage(messages.friends) }}
		</h3>
		<IconButton
			v-if="incomingRequests.length > 0"
			v-tooltip="formatMessage(messages.viewFriendRequests, { count: incomingRequests.length })"
			type="quiet"
			:label="formatMessage(messages.viewFriendRequests, { count: incomingRequests.length })"
			class="relative"
			@click="friendInvitesModal.show"
		>
			<MailIcon />
			<span
				v-if="incomingRequests.length > 0"
				aria-hidden="true"
				class="absolute bg-brand text-brand-inverted text-[8px] top-0.5 px-1 right-0.5 min-w-3 h-3 rounded-full flex items-center justify-center font-bold"
			>
				{{ incomingRequests.length }}
			</span>
		</IconButton>
	</div>
	<div class="flex flex-col gap-3">
		<h3 v-if="loading" class="text-base text-primary font-medium m-0">
			{{ formatMessage(messages.friends) }}
		</h3>
		<template v-if="loading">
			<div v-for="n in 5" :key="n" class="flex gap-2 items-center animate-pulse">
				<div class="min-w-9 min-h-9 bg-button-bg rounded-full"></div>
				<div class="flex flex-col w-full">
					<div class="h-3 bg-button-bg rounded-full w-1/2 mb-1"></div>
					<div class="h-2.5 bg-button-bg rounded-full w-3/4"></div>
				</div>
			</div>
		</template>
		<template v-else-if="sortedFriends.length === 0">
			<div class="text-sm">
				<div v-if="!userCredentials">
					<IntlFormatted :message-id="messages.signInToAddFriends">
						<template #link="{ children }">
							<span class="font-semibold text-brand cursor-pointer" @click="signIn">
								<component :is="() => children" />
							</span>
						</template>
					</IntlFormatted>
				</div>
				<div v-else>
					<IntlFormatted :message-id="messages.addFriendsToShare">
						<template #link="{ children }">
							<span class="font-semibold text-brand cursor-pointer" @click="addFriendModal.show">
								<component :is="() => children" />
							</span>
						</template>
					</IntlFormatted>
				</div>
			</div>
		</template>
		<template v-else>
			<FriendsSection
				v-if="activeFriends.length > 0"
				:is-searching="!!search"
				:open-by-default="!isFriendsSectionCollapsed('friends_active_collapsed')"
				:friends="activeFriends"
				:heading="formatMessage(messages.active)"
				:remove-friend="removeFriend"
				@on-open="setFriendsSectionCollapsed('friends_active_collapsed', false)"
				@on-close="setFriendsSectionCollapsed('friends_active_collapsed', true)"
			/>
			<FriendsSection
				v-if="onlineFriends.length > 0"
				:is-searching="!!search"
				:open-by-default="!isFriendsSectionCollapsed('friends_online_collapsed')"
				:friends="onlineFriends"
				:heading="formatMessage(messages.online)"
				:remove-friend="removeFriend"
				@on-open="setFriendsSectionCollapsed('friends_online_collapsed', false)"
				@on-close="setFriendsSectionCollapsed('friends_online_collapsed', true)"
			/>
			<FriendsSection
				v-if="offlineFriends.length > 0"
				:is-searching="!!search"
				:open-by-default="!isFriendsSectionCollapsed('friends_offline_collapsed')"
				:friends="offlineFriends"
				:heading="formatMessage(messages.offline)"
				:remove-friend="removeFriend"
				@on-open="setFriendsSectionCollapsed('friends_offline_collapsed', false)"
				@on-close="setFriendsSectionCollapsed('friends_offline_collapsed', true)"
			/>
			<FriendsSection
				v-if="pendingFriends.length > 0"
				:is-searching="!!search"
				:open-by-default="!isFriendsSectionCollapsed('friends_pending_collapsed')"
				:friends="pendingFriends"
				:heading="formatMessage(messages.pending)"
				:remove-friend="removeFriend"
				@on-open="setFriendsSectionCollapsed('friends_pending_collapsed', false)"
				@on-close="setFriendsSectionCollapsed('friends_pending_collapsed', true)"
			/>
			<p v-if="filteredFriends.length === 0 && search" class="text-sm text-secondary my-1 mx-4">
				{{ formatMessage(messages.noFriendsMatch, { query: search }) }}
			</p>
		</template>
	</div>
</template>
