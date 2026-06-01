<script setup lang="ts">
import { Avatar, ButtonStyled } from '@modrinth/ui'
import { computed, ref } from 'vue'

import { useSocial } from '@/composables/useSocial'

defineOptions({ name: 'CoreFriendsPage' })

const { friends, error, sendFriendRequest, respondFriendRequest, removeFriend } = useSocial()

const codeOrName = ref('')
const sending = ref(false)
const sentNote = ref<string | null>(null)

const friendList = computed(() => friends.value?.friends ?? [])
const incoming = computed(() => friends.value?.incoming ?? [])
const outgoing = computed(() => friends.value?.outgoing ?? [])

async function add() {
	const value = codeOrName.value.trim()
	if (!value) return
	sending.value = true
	sentNote.value = null
	const looksLikeCode = value.toUpperCase().startsWith('AMB-')
	await sendFriendRequest(looksLikeCode ? { friendCode: value } : { username: value })
	sending.value = false
	if (!error.value) {
		sentNote.value = 'Friend request sent.'
		codeOrName.value = ''
	}
}
</script>

<template>
	<div class="flex flex-col gap-4">
		<div class="rounded-2xl bg-bg-raised p-4 flex flex-col gap-2">
			<span class="font-bold">Add a friend</span>
			<span class="text-secondary text-sm">Enter a friend code (AMB-XXXXXX) or a username.</span>
			<div class="flex gap-2">
				<input
					v-model="codeOrName"
					class="flex-1 rounded-lg bg-bg-input px-3 py-2"
					placeholder="AMB-ABC123 or username"
					@keyup.enter="add"
				/>
				<ButtonStyled color="brand">
					<button :disabled="sending" @click="add">
						{{ sending ? 'Sending…' : 'Add friend' }}
					</button>
				</ButtonStyled>
			</div>
			<span v-if="sentNote" class="text-green text-sm">{{ sentNote }}</span>
			<span v-if="error" class="text-red text-sm">{{ error.message }}</span>
		</div>

		<div v-if="incoming.length > 0" class="rounded-2xl bg-bg-raised p-4 flex flex-col gap-3">
			<span class="font-bold">Incoming requests ({{ incoming.length }})</span>
			<div
				v-for="entry in incoming"
				:key="entry.request._id"
				class="flex items-center justify-between gap-2"
			>
				<div class="flex items-center gap-2">
					<Avatar :src="entry.user?.image" :alt="entry.user?.username" size="32px" circle />
					<span>{{ entry.user?.displayName ?? entry.user?.username ?? 'Unknown user' }}</span>
				</div>
				<div class="flex gap-2">
					<ButtonStyled color="brand">
						<button @click="respondFriendRequest(entry.request._id, true)">Accept</button>
					</ButtonStyled>
					<ButtonStyled>
						<button @click="respondFriendRequest(entry.request._id, false)">Decline</button>
					</ButtonStyled>
				</div>
			</div>
		</div>

		<div v-if="outgoing.length > 0" class="rounded-2xl bg-bg-raised p-4 flex flex-col gap-2">
			<span class="font-bold">Sent requests ({{ outgoing.length }})</span>
			<div
				v-for="entry in outgoing"
				:key="entry.request._id"
				class="flex items-center gap-2 text-secondary"
			>
				<Avatar :src="entry.user?.image" :alt="entry.user?.username" size="24px" circle />
				<span
					>{{ entry.user?.displayName ?? entry.user?.username ?? 'Unknown user' }} · pending</span
				>
			</div>
		</div>

		<div class="rounded-2xl bg-bg-raised p-4 flex flex-col gap-3">
			<span class="font-bold">Friends ({{ friendList.length }})</span>
			<p v-if="friendList.length === 0" class="m-0 text-secondary text-sm">
				No friends yet. Add someone with their friend code above.
			</p>
			<div
				v-for="friend in friendList"
				:key="friend.friendshipId"
				class="flex items-center justify-between gap-2"
			>
				<div class="flex items-center gap-2">
					<Avatar :src="friend.user?.image" :alt="friend.user?.username" size="32px" circle />
					<span>{{ friend.user?.displayName ?? friend.user?.username }}</span>
				</div>
				<ButtonStyled size="small">
					<button @click="removeFriend(friend.user!.userId)">Remove</button>
				</ButtonStyled>
			</div>
		</div>
	</div>
</template>
